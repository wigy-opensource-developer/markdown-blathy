// Tauri shell bootstrap. Per the spec, `markdown/` and `typst_engine/` are
// plain Rust modules (no `tauri` imports) registered as thin `#[tauri::command]`
// wrappers in `commands.rs`. The dialog plugin is registered for the file
// open/save/export commands that build on it (M4).
// Desktop-only: Linux + macOS.

pub mod commands;
pub mod files;
pub mod links;
pub mod markdown;
pub mod options;
pub mod typst_engine;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::render_preview,
            commands::open_markdown,
            commands::save_markdown,
            commands::export_pdf,
            commands::get_initial_directory,
            commands::open_external,
            commands::validate_template,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
