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
/// A free OpenRouter model is used by default. Model availability on OpenRouter
/// changes over time, so this is fully overridable from the Settings screen.
pub const DEFAULT_MODEL: &str = "google/gemma-2-9b-it:free";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub endpoint: String,
    pub model: String,
    pub default_target_language: String,
    pub temperature: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
            default_target_language: "English".to_string(),
            temperature: 0.3,
        }
    }
}

impl Settings {
    /// Load settings from `<config_dir>/settings.json`, falling back to defaults
    /// when the file is absent or unreadable.
    pub fn load(config_dir: &Path) -> Self {
        let path = config_dir.join(SETTINGS_FILE);
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
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
