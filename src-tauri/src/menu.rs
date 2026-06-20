//! Native application menu mirroring the in-app toolbar.
//!
//! Custom items emit a `"menu"` event whose payload is the item id; the frontend
//! dispatches each to the SAME handler as the toolbar, so the native menu bar
//! and the in-app UI stay consistent. Clipboard and window items use native
//! predefined behaviour. Custom items deliberately carry NO accelerators — the
//! keyboard shortcuts are owned by the frontend (`useShortcuts`), so there is no
//! double-firing; the menu adds click parity and discoverability.

use tauri::menu::{AboutMetadata, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{App, Runtime};

pub fn build<R: Runtime>(app: &App<R>) -> tauri::Result<Menu<R>> {
    // App menu (shows as the app menu on macOS).
    let settings = MenuItemBuilder::with_id("settings", "Settings…").build(app)?;
    let app_menu = SubmenuBuilder::new(app, "AIX Text Editor")
        .about(Some(AboutMetadata {
            name: Some("AIX Text Editor".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            copyright: Some(
                "Copyright (c) 2026 Satoshi Kume. Artistic License 2.0.".into(),
            ),
            ..Default::default()
        }))
        .separator()
        .item(&settings)
        .separator()
        .hide()
        .quit()
        .build()?;

    // File
    let new_tab = MenuItemBuilder::with_id("new_tab", "New Tab").build(app)?;
    let open = MenuItemBuilder::with_id("open", "Open…").build(app)?;
    let save = MenuItemBuilder::with_id("save", "Save").build(app)?;
    let import = MenuItemBuilder::with_id("import", "Import…").build(app)?;
    let export_txt = MenuItemBuilder::with_id("export_txt", "Export as .txt").build(app)?;
    let export_md = MenuItemBuilder::with_id("export_md", "Export as .md").build(app)?;
    let export_rtf = MenuItemBuilder::with_id("export_rtf", "Export as .rtf").build(app)?;
    let export = SubmenuBuilder::new(app, "Export")
        .item(&export_txt)
        .item(&export_md)
        .item(&export_rtf)
        .build()?;
    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_tab)
        .item(&open)
        .item(&save)
        .separator()
        .item(&import)
        .item(&export)
        .build()?;

    // Edit — Undo/Redo are routed to the app's own history; clipboard is native.
    let undo = MenuItemBuilder::with_id("undo", "Undo").build(app)?;
    let redo = MenuItemBuilder::with_id("redo", "Redo").build(app)?;
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&undo)
        .item(&redo)
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // AI
    let draft = MenuItemBuilder::with_id("draft", "Draft from theme…").build(app)?;
    let analyze = MenuItemBuilder::with_id("analyze", "Analyze relationships").build(app)?;
    let ai_menu = SubmenuBuilder::new(app, "AI")
        .item(&draft)
        .item(&analyze)
        .build()?;

    // Window
    let window_menu = SubmenuBuilder::new(app, "Window")
        .minimize()
        .separator()
        .close_window()
        .build()?;

    MenuBuilder::new(app)
        .items(&[&app_menu, &file_menu, &edit_menu, &ai_menu, &window_menu])
        .build()
}
