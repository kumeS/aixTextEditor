//! Tauri command surface — the only entry points the frontend can invoke.
//!
//! These are plain application commands (not plugin commands), so they do not
//! require capability/ACL grants; registering them in `generate_handler!` is
//! sufficient. The API key is read here on the Rust side and never crosses to
//! the frontend.

use crate::ai::{self, AiRequest, LlmConfig};
use crate::deck;
use crate::error::{AppError, AppResult};
use crate::fileio;
use crate::models::{AnalysisResult, Document};
use crate::pptx;
use crate::settings::{self, Settings};
use serde::Serialize;
use std::path::PathBuf;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

fn config_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Could not resolve config directory: {e}")))
}

/// True for endpoints served from the local machine (e.g. an Ollama bridge),
/// which don't need an API key.
fn is_local_endpoint(endpoint: &str) -> bool {
    let e = endpoint.to_ascii_lowercase();
    e.contains("localhost")
        || e.contains("127.0.0.1")
        || e.contains("0.0.0.0")
        || e.contains("[::1]")
}

/// Resolve the API key for a request. Remote providers (OpenRouter, …) require a
/// key; local endpoints may run keyless, so an empty key is allowed there.
fn api_key_for(endpoint: &str) -> AppResult<String> {
    match settings::get_api_key()? {
        Some(k) if !k.trim().is_empty() => Ok(k),
        _ if is_local_endpoint(endpoint) => Ok(String::new()),
        _ => Err(AppError::Config(
            "No API key is set. Open Settings and add your OpenRouter API key. \
             (Local endpoints such as Ollama can leave the key blank.)"
                .to_string(),
        )),
    }
}

fn load_llm_config(app: &AppHandle) -> AppResult<LlmConfig> {
    let settings = Settings::load(&config_dir(app)?);
    let api_key = api_key_for(&settings.endpoint)?;
    Ok(LlmConfig {
        endpoint: settings.endpoint,
        model: settings.model,
        api_key,
        temperature: settings.temperature,
    })
}

/// Like `load_llm_config` but uses the configured IMAGE model.
fn load_image_llm_config(app: &AppHandle) -> AppResult<LlmConfig> {
    let settings = Settings::load(&config_dir(app)?);
    let api_key = api_key_for(&settings.endpoint)?;
    Ok(LlmConfig {
        endpoint: settings.endpoint,
        model: settings.image_model,
        api_key,
        temperature: settings.temperature,
    })
}

// ----- document lifecycle --------------------------------------------------

#[tauri::command]
pub fn import_document(path: String) -> AppResult<Document> {
    fileio::import_from_path(&path)
}

#[tauri::command]
pub fn export_document(document: Document, path: String, format: String) -> AppResult<()> {
    fileio::export_to_path(&document, &path, &format)
}

/// Export the document as a PowerPoint deck: derive slides from the document
/// (headings → slides, paragraphs → bullets, images embedded), download any
/// remote image URLs, write `.pptx`, and report anything that couldn't be added.
#[tauri::command]
pub async fn export_pptx(document: Document, path: String) -> AppResult<pptx::PptxReport> {
    let mut deck = deck::document_to_deck(&document);
    pptx::resolve_remote_images(&mut deck).await;
    let (bytes, warnings) = pptx::deck_to_pptx(&deck)?;
    std::fs::write(&path, bytes)?;
    Ok(pptx::PptxReport {
        slides: deck.slides.len(),
        warnings,
    })
}

/// Save/open the native `.aix` document format (the chunk JSON from spec §5).
#[tauri::command]
pub fn save_document_json(document: Document, path: String) -> AppResult<()> {
    std::fs::write(path, serde_json::to_string_pretty(&document)?)?;
    Ok(())
}

#[tauri::command]
pub fn open_document_json(path: String) -> AppResult<Document> {
    let s = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&s)?)
}

// ----- settings & secret storage ------------------------------------------

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppResult<Settings> {
    Ok(Settings::load(&config_dir(&app)?))
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> AppResult<()> {
    settings.save(&config_dir(&app)?)
}

#[tauri::command]
pub fn set_api_key(key: String) -> AppResult<()> {
    settings::set_api_key(&key)
}

#[tauri::command]
pub fn has_api_key() -> bool {
    matches!(settings::get_api_key(), Ok(Some(_)))
}

#[tauri::command]
pub fn delete_api_key() -> AppResult<()> {
    settings::delete_api_key()
}

// ----- AI ------------------------------------------------------------------

#[tauri::command]
pub async fn ai_process(app: AppHandle, request: AiRequest) -> AppResult<String> {
    let config = load_llm_config(&app)?;
    ai::run_action(&config, &request).await
}

/// Streaming variant of `ai_process`: pushes the accumulated text to the
/// `on_delta` channel as it grows and returns the final text.
#[tauri::command]
pub async fn ai_process_stream(
    app: AppHandle,
    request: AiRequest,
    on_delta: Channel<String>,
) -> AppResult<String> {
    let config = load_llm_config(&app)?;
    let ch = on_delta.clone();
    ai::run_action_stream(&config, &request, |text| {
        let _ = ch.send(text.to_string());
    })
    .await
}

fn draft_title(theme: &str) -> String {
    if theme.trim().is_empty() {
        "Untitled Document".to_string()
    } else {
        theme.trim().to_string()
    }
}

/// Streaming draft event pushed to the frontend channel.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DraftEvent {
    /// Live snapshot while generating (chunks carry stable position-based ids).
    Update { document: Document },
    /// Final document with real UUID chunk ids.
    Done { document: Document },
}

/// Generate a draft and stream the parsed document to the frontend as it grows.
#[tauri::command]
pub async fn ai_draft_stream(
    app: AppHandle,
    theme: String,
    target_words: Option<u32>,
    reference: Option<String>,
    on_event: Channel<DraftEvent>,
) -> AppResult<()> {
    let config = load_llm_config(&app)?;
    let settings = Settings::load(&config_dir(&app)?);
    let title = draft_title(&theme);

    let language = Some(settings.default_target_language.clone());
    let tone = Some(settings.writing_tone.clone());
    let title_cb = title.clone();
    let ch = on_event.clone();
    let full = ai::generate_draft_stream(
        &config,
        &theme,
        target_words,
        language.as_deref(),
        tone.as_deref(),
        reference.as_deref(),
        |text| {
        let mut doc = fileio::text_to_document(&title_cb, text);
        // Position-based ids so the frontend reconciles chunks in place during
        // streaming (instead of remounting the whole document each token).
        for (i, c) in doc.chunks.iter_mut().enumerate() {
            c.id = format!("draft-{i}");
        }
        let _ = ch.send(DraftEvent::Update { document: doc });
    })
    .await?;

    // Finalise with real UUIDs for a stable, editable document.
    let document = fileio::text_to_document(&title, &full);
    let _ = on_event.send(DraftEvent::Done { document });
    Ok(())
}

/// Generate an image from a prompt using the configured image model.
#[tauri::command]
pub async fn ai_generate_image(app: AppHandle, prompt: String) -> AppResult<String> {
    let config = load_image_llm_config(&app)?;
    ai::generate_image(&config, &prompt).await
}

#[tauri::command]
pub async fn ai_generate_diagram(
    app: AppHandle,
    text: String,
    instruction: Option<String>,
) -> AppResult<String> {
    let config = load_llm_config(&app)?;
    ai::generate_diagram(&config, &text, instruction.as_deref()).await
}

#[tauri::command]
pub async fn ai_analyze_document(app: AppHandle, document: Document) -> AppResult<AnalysisResult> {
    let config = load_llm_config(&app)?;
    ai::analyze_document(&config, &document).await
}

// ----- Draft reference material (txt/md/rtf/pdf + URL) ----------------------

/// Read a local file as plain reference text for the Draft feature.
#[tauri::command]
pub fn read_reference_file(path: String) -> AppResult<String> {
    fileio::read_reference_text(&path)
}

/// Fetch a URL and return its readable text (tags/scripts stripped), for use as
/// Draft reference material.
#[tauri::command]
pub async fn fetch_url_text(url: String) -> AppResult<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .header("User-Agent", "aixTextEditor/1.1 (+https://github.com/kumeS/AIX_Text_Editor)")
        .send()
        .await?;
    let status = res.status();
    if !status.is_success() {
        return Err(AppError::Network(format!(
            "Could not fetch URL (HTTP {}).",
            status.as_u16()
        )));
    }
    let body = res.text().await?;
    let text = strip_html(&body);
    Ok(text.chars().take(20_000).collect())
}

/// Crude but UTF-8-safe HTML→text: drop `<script>`/`<style>` blocks and all
/// tags, decode a few common entities, and collapse whitespace.
fn strip_html(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let len = html.len();
    let mut out = String::with_capacity(len / 2);
    let mut i = 0usize;
    while i < len {
        if lower[i..].starts_with("<script") || lower[i..].starts_with("<style") {
            let close = if lower[i..].starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            match lower[i..].find(close) {
                Some(rel) => i += rel + close.len(),
                None => break,
            }
            out.push(' ');
            continue;
        }
        let ch = html[i..].chars().next().unwrap();
        if ch == '<' {
            match html[i..].find('>') {
                Some(rel) => i += rel + 1,
                None => break,
            }
            out.push(' ');
            continue;
        }
        out.push(ch);
        i += ch.len_utf8();
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ----- Text-to-speech (read aloud) -----------------------------------------

/// Speak `text` aloud using the OS speech synthesizer (macOS `say`). Runs
/// detached so it never blocks the UI; an optional `voice` selects a specific
/// system voice (e.g. "Kyoko" for Japanese). Any current speech is stopped first.
#[tauri::command]
pub fn speak_text(text: String, voice: Option<String>) -> AppResult<()> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        // Stop any in-flight speech so a new request restarts cleanly.
        let _ = Command::new("killall").arg("say").status();
        let mut cmd = Command::new("say");
        // Only pass the requested voice if it's actually installed; otherwise the
        // system default is used (better than `say` erroring on a missing voice).
        if let Some(v) = voice
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .filter(|v| macos_voice_installed(v))
        {
            cmd.arg("-v").arg(v);
        }
        // Read the text from stdin to avoid argv length/flag-parsing limits.
        let mut child = cmd
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Other(format!("Could not start speech: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(trimmed.as_bytes());
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = voice;
        Err(AppError::Other(
            "Read-aloud is currently available on macOS only.".to_string(),
        ))
    }
}

/// True if a macOS `say` voice with this name is installed (checks `say -v '?'`).
#[cfg(target_os = "macos")]
fn macos_voice_installed(name: &str) -> bool {
    use std::process::Command;
    let Ok(out) = Command::new("say").arg("-v").arg("?").output() else {
        return false;
    };
    let listing = String::from_utf8_lossy(&out.stdout);
    let needle = name.to_ascii_lowercase();
    // Each line begins with the voice name, e.g. "Kyoko    ja_JP  # …".
    listing.lines().any(|line| {
        line.split_whitespace()
            .next()
            .map(|w| w.to_ascii_lowercase() == needle)
            .unwrap_or(false)
    })
}

/// Stop any in-progress read-aloud.
#[tauri::command]
pub fn stop_speaking() -> AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("killall").arg("say").status();
    }
    Ok(())
}
