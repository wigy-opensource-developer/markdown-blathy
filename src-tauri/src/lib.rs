// Tauri shell bootstrap. Per the spec, `markdown/` and `typst_engine/` are
// plain Rust modules (no `tauri` imports) registered as thin `#[tauri::command]`
// wrappers in `commands.rs`. The dialog plugin is registered for the file
// open/save/export commands that build on it (M4).
// Desktop-only: Linux + macOS.

pub mod commands;
pub mod engine_cache;
pub mod files;
pub mod links;
pub mod markdown;
pub mod options;
pub mod typst_engine;

/// Reads `RUST_LOG` from the environment; defaults to `warn` when unset.
/// Must be called once before the `tauri::Builder` is constructed.
pub fn init_logging() {
    use tracing_subscriber::EnvFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();
}

pub fn run() {
    init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // M6 engine cache (§7.5): one compiled engine per static source,
        // rebuilt (and fonts re-scanned) only when the template/options change.
        // Arc so commands can hand an owned handle to the blocking thread.
        .manage(std::sync::Arc::new(std::sync::Mutex::new(
            engine_cache::TemplateEngineCache::new(),
        )))
        .invoke_handler(tauri::generate_handler![
            commands::render_preview,
            commands::open_markdown,
            commands::save_markdown,
            commands::export_pdf,
            commands::get_initial_directory,
            commands::open_external,
            commands::update_front_matter,
            commands::validate_template,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::init_logging;

    /// `init_logging()` must not panic — a panic here crashes the app
    /// before the UI even renders. The default filter is `warn` when
    /// `RUST_LOG` is not set.
    #[test]
    fn init_logging_does_not_panic_with_default_filter() {
        // Clear RUST_LOG to ensure we test the default path.
        std::env::remove_var("RUST_LOG");
        // init_logging may be called only once (tracing::init panics on
        // second init), so we run it in a child process via a helper.
        // For now, just verify it doesn't panic when called fresh.
        init_logging();
    }
}
