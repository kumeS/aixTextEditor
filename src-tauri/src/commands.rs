//! Tauri command surface — the only entry points the frontend can invoke.
//!
//! These are plain application commands (not plugin commands), so they do not
//! require capability/ACL grants; registering them in `generate_handler!` is
//! sufficient. The API key is read here on the Rust side and never crosses to
//! the frontend.

use crate::ai::{self, AiRequest, AnalysisResult, LlmConfig};
use crate::error::{AppError, AppResult};
use crate::fileio;
use crate::models::{Chunk, Document};
use crate::settings::{self, Settings};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

fn config_dir(app: &AppHandle) -> AppResult<PathBuf> {
    app.path()
        .app_config_dir()
        .map_err(|e| AppError::Config(format!("Could not resolve config directory: {e}")))
}

fn load_llm_config(app: &AppHandle) -> AppResult<LlmConfig> {
    let settings = Settings::load(&config_dir(app)?);
    let api_key = settings::get_api_key()?.ok_or_else(|| {
        AppError::Config(
            "No API key is set. Open Settings and add your OpenRouter API key.".to_string(),
        )
    })?;
    Ok(LlmConfig {
        endpoint: settings.endpoint,
        model: settings.model,
        api_key,
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

/// Generate a full document draft on a theme, split into paragraph + heading chunks.
#[tauri::command]
pub async fn ai_draft(app: AppHandle, theme: String) -> AppResult<Document> {
    let config = load_llm_config(&app)?;
    let markdown = ai::generate_draft(&config, &theme).await?;
    let title = if theme.trim().is_empty() {
        "Untitled Document".to_string()
    } else {
        theme.trim().to_string()
    };
    Ok(fileio::text_to_document(&title, &markdown))
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
