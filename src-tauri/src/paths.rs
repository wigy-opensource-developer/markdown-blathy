//! Tauri resource and app-data path resolution.
//!
//! These paths come from the running Tauri application, so they stay in the
//! adapter. The core receives the resolved template directory through
//! `Service::new` and never discovers it itself.

use std::path::PathBuf;

use markdown_blathy_core::files;
use tauri::{AppHandle, Manager};

/// The directory holding the bundled templates (one subdirectory per template,
/// each with a `main.typ`). Resolved from the Tauri resource dir; in dev
/// (`cargo run`/`tauri dev`) the resources are copied by the build script, and
/// the core's source-tree fallback covers a missing resource directory.
pub(crate) fn templates_dir(app: &AppHandle) -> Option<PathBuf> {
    files::templates_dir(app.path().resource_dir().ok())
}

/// The app-data directory, created on demand by callers that persist state.
pub(crate) fn app_data_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok()
}
