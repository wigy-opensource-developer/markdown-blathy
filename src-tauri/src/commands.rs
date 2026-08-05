//! Thin `#[tauri::command]` wrappers (§7.4 of SPEC.md).
//!
//! All real logic lives in the plain modules (`markdown/`, `typst_engine/`,
//! `files/`) which have no `tauri` imports; these functions are
//! serialization-only shims plus the system dialogs.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::files;
use crate::options::RenderOptions;
use crate::typst_engine::engine;
use crate::typst_engine::template::{self, resolve_template_source};

/// Frontend → Rust payload for `render_preview` (§7.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPayload {
    pub markdown: String,
    pub options: RenderOptions,
    /// Monotonically increasing request id for the stale-response guard (§7.5).
    pub seq: u64,
}

/// Per-page SVG strings plus warnings, echoed back with the request's `seq`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOutcome {
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
    pub seq: u64,
}

/// Result of a dialog-bearing file command (§7.4, M4).
///
/// Serializes as `{ "path", "content" }` on success and as
/// `{ "canceled": true }` when the user dismissed the dialog — `canceled`
/// is silent in the UI (§9).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOutcome {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub canceled: bool,
}

impl FileOutcome {
    /// The user dismissed the dialog — resolve, not an error (§9).
    fn canceled() -> Self {
        Self {
            path: None,
            content: None,
            canceled: true,
        }
    }

    /// `open_markdown` success: the file path plus its contents.
    fn opened(path: impl AsRef<std::path::Path>, content: String) -> Self {
        Self {
            path: Some(path.as_ref().to_string_lossy().into_owned()),
            content: Some(content),
            canceled: false,
        }
    }

    /// `save_markdown` / `export_pdf` success: just the written path.
    fn saved(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            path: Some(path.as_ref().to_string_lossy().into_owned()),
            content: None,
            canceled: false,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !b
}

/// Live preview: markdown + options → per-page SVG strings (§7.4).
///
/// Compilation is CPU-bound, so it runs on `spawn_blocking` to keep the IPC
/// handler off the async runtime's critical path. `seq` is echoed back so the
/// frontend can drop stale responses.
#[tauri::command]
pub async fn render_preview(payload: RenderPayload) -> Result<PreviewOutcome, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let source = template::build_source(&template_source, &payload.options, &payload.markdown)?;

    let outcome = tauri::async_runtime::spawn_blocking(move || engine::compile_preview(&source))
        .await
        .map_err(|e| format!("Preview task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    Ok(PreviewOutcome {
        pages: outcome.pages,
        warnings: outcome.warnings,
        seq: payload.seq,
    })
}

/// Export: markdown + options → system save dialog (defaulting to `<title>.pdf`
/// in `directory`) → write the PDF atomically → open it in the system viewer
/// (§7.4, M4).
///
/// Compilation is CPU-bound, so it runs on `spawn_blocking`; the save dialog
/// blocks too, which is fine off the UI thread.
#[tauri::command]
pub async fn export_pdf(
    app: AppHandle,
    payload: RenderPayload,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let source = template::build_source(&template_source, &payload.options, &payload.markdown)?;

    let pdf = tauri::async_runtime::spawn_blocking(move || engine::compile_pdf(&source))
        .await
        .map_err(|e| format!("Export task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let mut dialog = app
        .dialog()
        .file()
        .set_title("Export PDF")
        .add_filter("PDF", &["pdf"])
        .set_file_name(format!("{title}.pdf"));
    if let Some(dir) = non_empty(&directory) {
        dialog = dialog.set_directory(dir);
    }
    let picked = dialog.blocking_save_file();
    let Some(path) = dialog_path(picked)? else {
        return Ok(FileOutcome::canceled());
    };

    files::write_atomically(&path, &pdf).map_err(|e| format!("Failed to write PDF: {e}"))?;
    open_in_viewer(&path);
    Ok(FileOutcome::saved(&path))
}

/// Open a Markdown file via the system dialog (starting in `directory` when
/// given) and read it (§7.4, M4).
#[tauri::command]
pub async fn open_markdown(
    app: AppHandle,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Open Markdown")
        .add_filter("Markdown", &["md", "markdown"])
        .add_filter("All files", &["*"]);
    if let Some(dir) = non_empty(&directory) {
        dialog = dialog.set_directory(dir);
    }
    let picked = dialog.blocking_pick_file();
    let Some(path) = dialog_path(picked)? else {
        return Ok(FileOutcome::canceled());
    };

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))?;
    Ok(FileOutcome::opened(&path, content))
}

/// Save Markdown via a "Save As…" dialog: the filename defaults to
/// `<title>.md` in `directory`, and the write is atomic (§7.4, M4).
#[tauri::command]
pub async fn save_markdown(
    app: AppHandle,
    content: String,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Save Markdown As…")
        .add_filter("Markdown", &["md"])
        .set_file_name(format!("{title}.md"));
    if let Some(dir) = non_empty(&directory) {
        dialog = dialog.set_directory(dir);
    }
    let picked = dialog.blocking_save_file();

    let Some(path) = dialog_path(picked)? else {
        return Ok(FileOutcome::canceled());
    };

    files::write_atomically(&path, content.as_bytes())
        .map_err(|e| format!("Failed to save: {e}"))?;
    Ok(FileOutcome::saved(&path))
}

/// The working directory at startup: `$XDG_DATA_HOME` when set and non-empty,
/// else `$HOME` (SPEC §8.4).
#[tauri::command]
pub fn get_initial_directory() -> String {
    files::initial_directory(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
        .to_string_lossy()
        .into_owned()
}

/// Open an http(s) URL in the system default browser (§7.4, M4).
///
/// Preview links must never navigate the webview: only http(s) is forwarded
/// to the browser; every other scheme is rejected with an error (§9).
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    if !crate::links::is_browser_url(&url) {
        return Err(format!(
            "Blocked external link: only http(s) URLs can be opened in the browser — got: {url}"
        ));
    }
    open::that(&url).map_err(|e| format!("Failed to open browser: {e}"))
}

/// `Some(&str)` for a non-empty string, else `None` — used to skip
/// `set_directory` on dialogs when no working directory is known yet.
fn non_empty(s: &Option<String>) -> Option<&str> {
    s.as_deref().filter(|s| !s.is_empty())
}

/// Convert a blocking dialog result into a `PathBuf`, mapping the (rare)
/// URL/error cases to a user-presentable message (§9). `None` means the user
/// dismissed the dialog.
fn dialog_path(
    picked: Option<tauri_plugin_dialog::FilePath>,
) -> Result<Option<std::path::PathBuf>, String> {
    match picked {
        Some(p) => p
            .into_path()
            .map(Some)
            .map_err(|e| format!("Failed to resolve path: {e}")),
        None => Ok(None),
    }
}

/// Open a file in the platform's default viewer (M4 acceptance: "Export
/// writes a PDF that opens in a viewer"). Non-fatal: the export already
/// succeeded on disk even if no viewer can be launched.
fn open_in_viewer(path: &Path) {
    if let Err(err) = open::that(path) {
        eprintln!("Failed to open PDF in viewer: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::{FileOutcome, PreviewOutcome, RenderPayload};
    use crate::options::{Flavor, Margins, Paper, TextAlign};

    /// Pins the IPC contract: the exact JSON the frontend `lib/api.js` sends
    /// (§8.4) must deserialize into `RenderPayload`.
    #[test]
    fn render_payload_deserializes_frontend_shape() {
        let json = r##"{
            "markdown": "# Hi",
            "options": {
                "flavor": "gfm",
                "paper": "a4",
                "margins": { "topMm": 25, "bottomMm": 25, "leftMm": 20, "rightMm": 20 },
                "textAlign": "left",
                "templatePath": null
            },
            "seq": 7
        }"##;
        let payload: RenderPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.markdown, "# Hi");
        assert_eq!(payload.seq, 7);
        assert_eq!(payload.options.flavor, Flavor::Gfm);
        assert_eq!(payload.options.paper, Paper::A4);
        assert_eq!(
            payload.options.margins,
            Margins {
                top_mm: 25.0,
                bottom_mm: 25.0,
                left_mm: 20.0,
                right_mm: 20.0,
            }
        );
        assert_eq!(payload.options.text_align, TextAlign::Left);
        assert!(payload.options.template_path.is_none());
    }

    /// Pins the IPC contract: `PreviewOutcome` serializes back to the
    /// camelCase shape the frontend reads (§7.4).
    #[test]
    fn preview_outcome_serializes_with_camel_case_and_seq() {
        let outcome = PreviewOutcome {
            pages: vec!["<svg>".into()],
            warnings: vec!["font warning".into()],
            seq: 3,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"pages\":[\"<svg>\"]"), "{json}");
        assert!(json.contains("\"warnings\":[\"font warning\"]"), "{json}");
        assert!(json.contains("\"seq\":3"), "{json}");
    }

    /// Success outcome must not carry a `canceled` flag — the frontend reads
    /// `path`/`content` (§8.4) and the absence of `canceled` means "done".
    #[test]
    fn file_outcome_serializes_success_without_canceled() {
        let outcome = FileOutcome {
            path: Some("/tmp/doc.md".into()),
            content: Some("# Hi".into()),
            canceled: false,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"path\":\"/tmp/doc.md\""), "{json}");
        assert!(json.contains("\"content\":\"# Hi\""), "{json}");
        assert!(
            !json.contains("canceled"),
            "success must omit canceled: {json}"
        );
    }

    /// A dismissed dialog serializes to exactly `{ "canceled": true }` —
    /// no `path`/`content` keys, so the frontend treats it as silent (§9).
    #[test]
    fn file_outcome_serializes_cancel_without_path() {
        let outcome = FileOutcome {
            path: None,
            content: None,
            canceled: true,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert_eq!(json, "{\"canceled\":true}");
    }
}
