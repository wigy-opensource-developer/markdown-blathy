// Tauri shell bootstrap. The render/convert/export logic lives in the
// Tauri-free `markdown-blathy-core` crate; `commands.rs` is a serialization,
// dialog and process-launch adapter over `markdown_blathy_core::Service`.
// Desktop app: Linux is the primary target; macOS and Windows are
// secondary.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::Emitter;

use markdown_blathy_core::service::Service;
use markdown_blathy_core::{options, typst_engine::template};

pub mod commands;
pub mod dialogs;
pub mod document;
pub mod paths;
pub mod prewarm;
pub mod recovery;
pub mod requests;
pub mod startup;

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

pub fn run(startup_file: Option<PathBuf>) {
    init_logging();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(startup::StartupDocument::new(startup_file))
        // The exact PathBuf of the open document, so save-in-place never goes
        // through the lossy IPC display string (SPEC §6, M9 task 2).
        .manage(document::DocumentPath::default())
        // One-shot allowance for a close the frontend already approved.
        .manage(requests::CloseGate::default())
        // M6 engine cache (§7.5) lives inside the core Service: one compiled
        // engine per static source, rebuilt (and fonts re-scanned) only when
        // the template/options change. Arc so commands can hand an owned
        // handle to a blocking thread.
        //
        // §7.5 engine prewarm: build the engine the first render will want
        // while the webview boots (under the boot loader), instead of under
        // the first preview request. The remembered static source
        // (`prewarm.json`, written after each successful render) covers
        // returning users with persisted options; a first launch falls back
        // to the fresh-install defaults. A miss only costs the (background)
        // rebuild on the first render — same as before the prewarm existed.
        .setup(|app| {
            use tauri::Manager;
            let templates_dir = paths::templates_dir(app.handle());
            let service = Arc::new(Service::new(templates_dir));
            app.manage(service.clone());

            let hint_file = paths::app_data_dir(app.handle()).map(|dir| prewarm::hint_path(&dir));
            // Resolved here (outside spawn_blocking) because `app` is not Send.
            tauri::async_runtime::spawn_blocking(move || {
                let (source, fonts) = match hint_file.as_deref().and_then(prewarm::load_hint) {
                    Some(hint) => (hint.static_source, hint.fonts),
                    None => {
                        let options = options::RenderOptions::frontend_defaults();
                        match template::prepare_render("", &options, service.templates_dir(), None)
                        {
                            Ok(prepared) => (prepared.static_source, prepared.fonts),
                            Err(e) => {
                                tracing::debug!("engine prewarm: prepare failed: {e}");
                                return;
                            }
                        }
                    }
                };
                match service.warm_engine(&source, &fonts) {
                    Ok(()) => tracing::debug!("engine prewarmed"),
                    Err(e) => tracing::debug!("engine prewarm: build failed: {e}"),
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::render_preview,
            commands::open_markdown,
            commands::open_pending_document,
            commands::frontend_ready,
            commands::confirm_close,
            commands::save_markdown,
            commands::save_document,
            commands::forget_document,
            commands::save_recovery_snapshot,
            commands::take_recovery,
            commands::restore_recovery,
            commands::discard_recovery,
            commands::clear_recovery,
            commands::export_pdf,
            commands::get_initial_directory,
            commands::open_external,
            commands::update_front_matter,
            commands::validate_template,
            commands::list_templates,
        ])
        // The frontend owns the unsaved-changes decision: intercept every
        // close and ask it (M9 task 3). `confirm_close` arms the one-shot
        // gate so the close it triggers passes straight through.
        .on_window_event(|window, event| {
            use tauri::Manager;
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let gate = window.state::<requests::CloseGate>();
                if gate.take_allowance() {
                    return;
                }
                api.prevent_close();
                if let Err(err) =
                    window.emit(requests::APP_REQUEST_EVENT, requests::AppRequest::Close)
                {
                    tracing::warn!(error = %err, "failed to ask the frontend about closing");
                }
            }
        })
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
