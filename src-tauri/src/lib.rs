//! AIX Text Editor — Tauri backend entry point.

mod ai;
mod commands;
mod error;
mod fileio;
mod models;
mod settings;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::new_document,
            commands::import_document,
            commands::export_document,
            commands::save_document_json,
            commands::open_document_json,
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::ai_process,
            commands::ai_draft,
            commands::ai_generate_diagram,
            commands::ai_analyze_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
