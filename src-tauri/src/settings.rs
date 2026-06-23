//! User settings + secure API-key storage.
//!
//! - Non-secret settings (endpoint, model, temperature, default target language)
//!   are persisted as JSON under the app config directory.
//! - The OpenRouter API key is stored in the OS-native secret store via `keyring`
//!   (macOS Keychain / Windows Credential Manager / Linux Secret Service). It is
//!   never written to disk in plaintext and never serialized to the frontend.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const KEYRING_SERVICE: &str = "com.aix.texteditor";
pub const KEYRING_ACCOUNT: &str = "openrouter-api-key";
pub const SETTINGS_FILE: &str = "settings.json";

pub const DEFAULT_ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";
/// The default text model. Model availability on OpenRouter changes over time,
/// so this is fully overridable from the Settings screen.
pub const DEFAULT_MODEL: &str = "deepseek/deepseek-v4-flash";

/// Default image-generation model. NOTE: image model ids change over time on
/// OpenRouter — these are starting points, fully editable from Settings.
pub const DEFAULT_IMAGE_MODEL: &str = "google/gemini-2.5-flash-image";

/// Starter list of selectable text models. Users add/remove their own from
/// Settings; ids may change over time on OpenRouter, so the list is editable.
fn default_models() -> Vec<String> {
    vec![
        DEFAULT_MODEL.to_string(), // deepseek/deepseek-v4-flash (default)
        "qwen/qwen3.6-flash".to_string(),
        "meta-llama/llama-4-maverick".to_string(),
        "moonshotai/kimi-k2.5".to_string(),
        "google/gemma-4-31b-it:free".to_string(),
        "meta-llama/llama-3.3-70b-instruct:free".to_string(),
        "deepseek/deepseek-r1:free".to_string(),
    ]
}

fn default_image_model() -> String {
    DEFAULT_IMAGE_MODEL.to_string()
}

/// Starter list of image-generation models (e.g. Google "Nano Banana"). Verify
/// the exact ids on openrouter.ai/models — edit/add from Settings.
fn default_image_models() -> Vec<String> {
    vec![
        DEFAULT_IMAGE_MODEL.to_string(), // Nano Banana (Gemini 2.5 Flash Image)
        "x-ai/grok-imagine-image-quality".to_string(),
        "recraft/recraft-v4-pro".to_string(),
        "openai/gpt-5.4-image-2".to_string(),
        "black-forest-labs/flux.2-klein-4b".to_string(),
        "google/gemini-3-pro-image-preview".to_string(), // Nano Banana Pro (verify id)
    ]
}

/// Default writing tone (empty = the AI's neutral academic default). The
/// Settings screen offers a small set of presets (blog / memo / report /
/// scientific / academic-paper); the chosen tone is applied to every writing
/// action so the whole document keeps a consistent voice.
fn default_writing_tone() -> String {
    String::new()
}

/// Best-effort default output language from the OS locale, so a Japanese (or
/// other non-English) user isn't forced to English out of the box. Used only
/// for a fresh install; fully overridable in Settings. Falls back to English.
fn default_language() -> String {
    let loc = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default()
        .to_lowercase();
    let lang = loc.split(['_', '.', '-']).next().unwrap_or("");
    match lang {
        "ja" => "日本語",
        "zh" => "中文",
        "ko" => "한국어",
        "es" => "Español",
        "fr" => "Français",
        "de" => "Deutsch",
        "pt" => "Português",
        "it" => "Italiano",
        "ru" => "Русский",
        "ar" => "العربية",
        _ => "English",
    }
    .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub endpoint: String,
    /// The active text model id used for requests.
    pub model: String,
    /// The user's selectable text-model list. `#[serde(default)]` keeps older
    /// settings files (without this field) loadable.
    #[serde(default = "default_models")]
    pub models: Vec<String>,
    /// The active image-generation model id.
    #[serde(default = "default_image_model")]
    pub image_model: String,
    /// The user's selectable image-model list.
    #[serde(default = "default_image_models")]
    pub image_models: Vec<String>,
    /// The default output language for ALL AI actions (translate target plus the
    /// language every other action writes its result in). Surfaced in Settings as
    /// "Default language" — see the prompt assembly in `ai.rs`.
    pub default_target_language: String,
    /// The global writing tone applied to writing actions (proofread/expand/…
    /// and drafts). `#[serde(default)]` keeps older settings files loadable.
    #[serde(default = "default_writing_tone")]
    pub writing_tone: String,
    pub temperature: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
            models: default_models(),
            image_model: default_image_model(),
            image_models: default_image_models(),
            default_target_language: default_language(),
            writing_tone: default_writing_tone(),
            temperature: 0.3,
        }
    }
}

impl Settings {
    /// Load settings from `<config_dir>/settings.json`, falling back to defaults
    /// when the file is absent or unreadable.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(SETTINGS_FILE);
        let mut settings: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        // Existing users have a saved `models`/`imageModels` list, so serde keeps
        // that list and the new built-in defaults never appear. Merge in any
        // pre-registered model that's missing, preserving the user's own
        // additions and ordering.
        settings.merge_default_models();
        settings
    }

    /// Append any built-in default model that isn't already in the list.
    fn merge_default_models(&mut self) {
        for m in default_models() {
            if !self.models.iter().any(|x| x == &m) {
                self.models.push(m);
            }
        }
        for m in default_image_models() {
            if !self.image_models.iter().any(|x| x == &m) {
                self.image_models.push(m);
            }
        }
    }

    pub fn save(&self, config_dir: &Path) -> AppResult<()> {
        std::fs::create_dir_all(config_dir)?;
        let path = config_dir.join(SETTINGS_FILE);
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

// ----- OS keychain helpers -------------------------------------------------

fn entry() -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).map_err(AppError::from)
}

pub fn set_api_key(key: &str) -> AppResult<()> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return delete_api_key();
    }
    entry()?.set_password(trimmed)?;
    Ok(())
}

pub fn get_api_key() -> AppResult<Option<String>> {
    match entry()?.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(AppError::Keyring(e.to_string())),
    }
}

pub fn delete_api_key() -> AppResult<()> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(AppError::Keyring(e.to_string())),
    }
}
