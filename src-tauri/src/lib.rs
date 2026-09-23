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
pub mod prewarm;
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
        // §7.5 engine prewarm: build the engine the first render will want
        // while the webview boots (under the boot loader), instead of under
        // the first preview request. The remembered static source
        // (`prewarm.json`, written after each successful render) covers
        // returning users with persisted options; a first launch falls back
        // to the fresh-install defaults. A miss only costs the (background)
        // rebuild on the first render — same as before the prewarm existed.
        .setup(|app| {
            use tauri::Manager;
            let cache = app
                .state::<std::sync::Arc<std::sync::Mutex<engine_cache::TemplateEngineCache>>>()
                .inner()
                .clone();
            let hint_file = app
                .path()
                .app_data_dir()
                .ok()
                .map(|dir| prewarm::hint_path(&dir));
            tauri::async_runtime::spawn_blocking(move || {
                let (source, fonts) = match hint_file.as_deref().and_then(prewarm::load_hint) {
                    Some(hint) => (hint.static_source, hint.fonts),
                    None => {
                        let options = options::RenderOptions::frontend_defaults();
                        match typst_engine::template::prepare_render("", &options) {
                            Ok(prepared) => (prepared.static_source, prepared.fonts),
                            Err(e) => {
                                tracing::debug!("engine prewarm: prepare failed: {e}");
                                return;
                            }
                        }
                    }
                };
                match cache.lock() {
                    Ok(mut cache) => match cache.engine_for(&source, &fonts) {
                        Ok(_) => tracing::debug!("engine prewarmed"),
                        Err(e) => tracing::debug!("engine prewarm: build failed: {e}"),
                    },
                    Err(_) => tracing::debug!("engine prewarm: cache lock poisoned"),
                }
            });
            Ok(())
        })
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
