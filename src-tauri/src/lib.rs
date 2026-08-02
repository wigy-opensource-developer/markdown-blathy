// Tauri shell bootstrap. Per the spec, `markdown/` and `typst_engine/` will be
// plain Rust modules (no `tauri` imports) registered as thin `#[tauri::command]`
// wrappers in `commands.rs`. For now (M0 scaffold) only the dialog plugin is
// registered, which the file open/save/export commands will build on.
// Desktop-only: Linux + macOS.

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
