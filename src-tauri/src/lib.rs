//! AIX Text Editor — Tauri backend entry point.

mod ai;
mod commands;
mod error;
mod fileio;
mod menu;
mod models;
mod settings;

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Native menu mirroring the toolbar; custom items emit a "menu" event.
            let menu = menu::build(app)?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                let _ = app.emit("menu", event.id().0.clone());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            commands::ai_draft_stream,
            commands::ai_generate_image,
            commands::ai_generate_diagram,
            commands::ai_analyze_document,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
