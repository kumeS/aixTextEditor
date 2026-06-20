//! Tauri command surface — the only entry points the frontend can invoke.
//!
//! These are plain application commands (not plugin commands), so they do not
//! require capability/ACL grants; registering them in `generate_handler!` is
//! sufficient. The API key is read here on the Rust side and never crosses to
//! the frontend.

use crate::ai::{self, AiRequest, LlmConfig};
use crate::error::{AppError, AppResult};
use crate::fileio;
use crate::models::{AnalysisResult, Chunk, Document};
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

fn require_api_key() -> AppResult<String> {
    settings::get_api_key()?.ok_or_else(|| {
        AppError::Config(
            "No API key is set. Open Settings and add your OpenRouter API key.".to_string(),
        )
    })
}

fn load_llm_config(app: &AppHandle) -> AppResult<LlmConfig> {
    let settings = Settings::load(&config_dir(app)?);
    Ok(LlmConfig {
        endpoint: settings.endpoint,
        model: settings.model,
        api_key: require_api_key()?,
        temperature: settings.temperature,
    })
}

/// Like `load_llm_config` but uses the configured IMAGE model.
fn load_image_llm_config(app: &AppHandle) -> AppResult<LlmConfig> {
    let settings = Settings::load(&config_dir(app)?);
    Ok(LlmConfig {
        endpoint: settings.endpoint,
        model: settings.image_model,
        api_key: require_api_key()?,
        temperature: settings.temperature,
    })
}

// ----- document lifecycle --------------------------------------------------

#[tauri::command]
pub fn new_document(title: Option<String>) -> Document {
    let mut doc = Document::new(&title.unwrap_or_else(|| "Untitled Document".to_string()));
    doc.chunks.push(Chunk::new_text(0, ""));
    doc
}

#[tauri::command]
pub fn import_document(path: String) -> AppResult<Document> {
    fileio::import_from_path(&path)
}

#[tauri::command]
pub fn export_document(document: Document, path: String, format: String) -> AppResult<()> {
    fileio::export_to_path(&document, &path, &format)
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

fn draft_title(theme: &str) -> String {
    if theme.trim().is_empty() {
        "Untitled Document".to_string()
    } else {
        theme.trim().to_string()
    }
}

/// Generate a full document draft on a theme, split into paragraph + heading chunks.
#[tauri::command]
pub async fn ai_draft(app: AppHandle, theme: String) -> AppResult<Document> {
    let config = load_llm_config(&app)?;
    let markdown = ai::generate_draft(&config, &theme).await?;
    Ok(fileio::text_to_document(&draft_title(&theme), &markdown))
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
    on_event: Channel<DraftEvent>,
) -> AppResult<()> {
    let config = load_llm_config(&app)?;
    let title = draft_title(&theme);

    let title_cb = title.clone();
    let ch = on_event.clone();
    let full = ai::generate_draft_stream(&config, &theme, |text| {
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
