//! System-dialog adapters.
//!
//! Tauri's blocking file dialogs are GUI-only; keeping them here lets the
//! commands stay focused on serialization and the core stay headless.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// A system *open* dialog with the given filters (name + extensions),
/// starting in `directory` when given. `None` means the user dismissed it —
/// the caller treats that as silent (§9).
pub(crate) fn pick_open_path(
    app: &AppHandle,
    title: &str,
    filters: &[(&str, &[&str])],
    directory: &Option<String>,
) -> Result<Option<PathBuf>, String> {
    let mut dialog = app.dialog().file().set_title(title);
    for (name, extensions) in filters {
        dialog = dialog.add_filter(*name, extensions);
    }
    if let Some(dir) = non_empty(directory) {
        dialog = dialog.set_directory(dir);
    }
    dialog_path(dialog.blocking_pick_file())
}

/// A system *save* dialog: title, one filter (name + extensions) and a default
/// file name, starting in `directory` when given. `None` means the user
/// dismissed it — the caller treats that as silent (§9).
pub(crate) fn pick_save_path(
    app: &AppHandle,
    title: &str,
    filter: (&str, &[&str]),
    file_name: String,
    directory: &Option<String>,
) -> Result<Option<PathBuf>, String> {
    let (name, extensions) = filter;
    let mut dialog = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(name, extensions)
        .set_file_name(file_name);
    if let Some(dir) = non_empty(directory) {
        dialog = dialog.set_directory(dir);
    }
    dialog_path(dialog.blocking_save_file())
}

/// `Some(&str)` for a non-empty string, else `None` — used to skip
/// `set_directory` on dialogs when no working directory is known yet.
fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// Convert a blocking dialog result into a `PathBuf`, mapping the (rare)
/// URL/error cases to a user-presentable message (§9). `None` means the user
/// dismissed the dialog.
fn dialog_path(picked: Option<tauri_plugin_dialog::FilePath>) -> Result<Option<PathBuf>, String> {
    match picked {
        Some(p) => p
            .into_path()
            .map(Some)
            .map_err(|e| format!("Failed to resolve path: {e}")),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dismissed dialog is `Ok(None)` — the command layer maps that to the
    /// silent `{ "canceled": true }` outcome (§9), never an error.
    #[test]
    fn dismissed_dialog_maps_to_none() {
        assert_eq!(dialog_path(None).unwrap(), None);
    }

    #[test]
    fn empty_directory_is_ignored() {
        assert_eq!(non_empty(&Some(String::new())), None);
        assert_eq!(non_empty(&None), None);
        assert_eq!(non_empty(&Some("/tmp".into())), Some("/tmp"));
    }
}
