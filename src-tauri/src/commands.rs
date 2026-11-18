//! Thin `#[tauri::command]` adapters (§7.4 of SPEC.md).
//!
//! All real logic lives in `markdown-blathy-core`; these functions do Tauri
//! serialization, system dialogs and process launching only. The core is
//! path/bytes based, so dialogs stay here and the CLI can reuse the same
//! operations.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use markdown_blathy_core::markdown::convert::DocMeta;
use markdown_blathy_core::markdown::frontmatter::Patch;
use markdown_blathy_core::options::RenderOptions;
use markdown_blathy_core::service::{RenderRequest, Service};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State, Window};
use tracing::{debug, info, instrument, warn};

use crate::dialogs::{pick_open_path, pick_save_path};
use crate::document::DocumentPath;
use crate::recovery::{self, RecoverySnapshot, RestoredSnapshot};
use crate::requests::{AppRequest, CloseGate, APP_REQUEST_EVENT};
use crate::startup::StartupDocument;

/// The recovery-snapshot directory inside the app-data dir (M9 task 4).
fn recovery_dir(app: &AppHandle) -> Option<PathBuf> {
    crate::paths::app_data_dir(app).map(|dir| dir.join("recovery"))
}

/// Frontend → Rust payload for `render_preview` (§7.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderPayload {
    pub markdown: String,
    pub options: RenderOptions,
    /// Monotonically increasing request id for the stale-response guard (§7.5).
    pub seq: u64,
    /// Absolute path of the document on disk, so document-relative images
    /// resolve against its folder (§6.4). None for the untitled sample.
    #[serde(default)]
    pub document_path: Option<String>,
}

/// Per-page SVG strings plus warnings and the effective document settings,
/// echoed back with the request's `seq`. The `doc` settings let the frontend
/// show the parsed front-matter title/author/flavor/extensions without
/// parsing YAML itself (§6.2).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOutcome {
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
    pub doc: DocMeta,
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

/// Convert the IPC payload into the core request shape.
fn render_request(payload: &RenderPayload) -> RenderRequest {
    RenderRequest {
        markdown: payload.markdown.clone(),
        options: payload.options.clone(),
        document_path: payload.document_path.as_deref().map(PathBuf::from),
    }
}

/// Live preview: markdown + options → per-page SVG strings (§7.4).
///
/// The core service owns the M6 engine cache; compilation is CPU-bound, so it
/// runs on `spawn_blocking`. `seq` is echoed back so the frontend can drop
/// stale responses. The engine identity returned by the service is saved as
/// the next launch's prewarm hint (§7.5).
#[tauri::command]
#[instrument(skip(state, app, payload), fields(seq = payload.seq, md_len = payload.markdown.len()))]
pub async fn render_preview(
    state: State<'_, Arc<Service>>,
    app: AppHandle,
    payload: RenderPayload,
) -> Result<PreviewOutcome, String> {
    let t0 = std::time::Instant::now();
    let request = render_request(&payload);
    let service = state.inner().clone();

    let detail = tauri::async_runtime::spawn_blocking(move || service.render_detailed(&request))
        .await
        .map_err(|e| format!("Preview task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let svg_bytes: usize = detail.result.pages.iter().map(|page| page.len()).sum();
    debug!(total_ms = t0.elapsed().as_millis(), "render timing (core)");
    info!(
        pages = detail.result.pages.len(),
        svg_bytes, "preview rendered"
    );

    // Prewarm hint for the next launch (§7.5): the static source just
    // rendered with is what the next launch's first render will want, so
    // setup() can build that engine under the boot loader. Saved on a
    // background thread, and a no-op while the file already holds it.
    let hint = crate::prewarm::PrewarmHint {
        static_source: detail.engine.static_source,
        fonts: detail.engine.fonts,
    };
    tauri::async_runtime::spawn_blocking(move || {
        let Some(dir) = crate::paths::app_data_dir(&app) else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = crate::prewarm::save_hint(&crate::prewarm::hint_path(&dir), &hint) {
            debug!("prewarm hint not saved: {e}");
        }
    });

    Ok(PreviewOutcome {
        pages: detail.result.pages,
        warnings: detail.result.warnings,
        doc: detail.result.doc,
        seq: payload.seq,
    })
}

/// Export: markdown + options → system save dialog (defaulting to `<title>.pdf`
/// in `directory`) → write the PDF atomically → open it in the system viewer
/// (§7.4, M4).
///
/// Shares the core engine cache with the preview, so exporting does not
/// re-scan fonts. Compilation and the blocking save dialog run off the UI
/// thread.
#[tauri::command]
#[instrument(skip(app, state, payload), fields(title, directory))]
pub async fn export_pdf(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    payload: RenderPayload,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let request = render_request(&payload);
    let service = state.inner().clone();
    let pdf = tauri::async_runtime::spawn_blocking(move || service.render_pdf_detailed(&request))
        .await
        .map_err(|e| format!("Export task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    // `FileOutcome` has no warnings channel (§8.4), so converter warnings
    // are logged rather than surfaced in the UI on export.
    if !pdf.warnings.is_empty() {
        warn!(warnings = ?pdf.warnings, "converter warnings during export");
    }

    let Some(path) = pick_save_path(
        &app,
        "Export PDF",
        ("PDF", &["pdf"]),
        format!("{title}.pdf"),
        &directory,
    )?
    else {
        info!(canceled = true, "export PDF canceled");
        return Ok(FileOutcome::canceled());
    };

    let service = state.inner().clone();
    let write_path = path.clone();
    let bytes = pdf.bytes;
    tauri::async_runtime::spawn_blocking(move || service.write_document(&write_path, &bytes))
        .await
        .map_err(|e| format!("Export write task failed: {e}"))?
        .map_err(|e| format!("Failed to write PDF: {e}"))?;
    open_in_viewer(&path);
    info!(path = %path.display(), "export PDF saved");
    Ok(FileOutcome::saved(&path))
}

/// Open a Markdown file via the system dialog (starting in `directory` when
/// given) and read it (§7.4, M4). The opened path becomes the document to
/// save in place (M9).
#[tauri::command]
#[instrument(skip(app, state, document), fields(directory))]
pub async fn open_markdown(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    document: State<'_, DocumentPath>,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let Some(path) = pick_open_path(
        &app,
        "Open Markdown",
        &[("Markdown", &["md", "markdown"]), ("All files", &["*"])],
        &directory,
    )?
    else {
        info!(canceled = true, "open markdown canceled");
        return Ok(FileOutcome::canceled());
    };

    let content = state
        .read_document(&path)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    document.set(&path);
    info!(path = %path.display(), "markdown opened");
    Ok(FileOutcome::opened(&path, content))
}

/// Read the document the CLI asked to open (M9). The frontend calls this
/// after its dirty guard passes, so an unsaved document is never replaced
/// silently. The path comes from the backend's exact `PathBuf`; nothing is
/// lost to the IPC display string.
///
/// `{ canceled: true }` means another caller already consumed the request.
#[tauri::command]
#[instrument(skip(state, document, pending))]
pub fn open_pending_document(
    state: State<'_, Arc<Service>>,
    document: State<'_, DocumentPath>,
    pending: State<'_, StartupDocument>,
) -> Result<FileOutcome, String> {
    let Some(path) = pending.take() else {
        return Ok(FileOutcome::canceled());
    };
    let content = state
        .read_document(&path)
        .map_err(|e| format!("Failed to read file: {e}"))?;
    document.set(&path);
    info!(path = %path.display(), "pending document opened");
    Ok(FileOutcome::opened(&path, content))
}

/// The frontend is listening for `app-request` (M9): emit the pending open
/// request, if any, and report whether one was sent. Emitting again on every
/// call is harmless — the request is only consumed by
/// `open_pending_document`, so a listener torn down between registration and
/// this call cannot lose the document.
#[tauri::command]
#[instrument(skip(pending))]
pub fn frontend_ready(window: Window, pending: State<'_, StartupDocument>) -> Result<bool, String> {
    if !pending.has() {
        return Ok(false);
    }
    window
        .emit(APP_REQUEST_EVENT, AppRequest::Open)
        .map_err(|e| format!("Failed to notify the frontend: {e}"))?;
    Ok(true)
}

/// The frontend approved closing the window (M9, Save/Discard): arm the
/// one-shot gate and close. The resulting `CloseRequested` sees the allowance
/// and does not ask again.
#[tauri::command]
#[instrument(skip(gate))]
pub fn confirm_close(window: Window, gate: State<'_, CloseGate>) -> Result<(), String> {
    gate.allow();
    window
        .close()
        .map_err(|e| format!("Failed to close the window: {e}"))
}

/// Save Markdown via a "Save As…" dialog: the filename defaults to
/// `<title>.md` in `directory`, and the write is atomic (§7.4, M4). The
/// chosen path becomes the document to save in place (M9).
#[tauri::command]
#[instrument(skip(app, state, document, content), fields(title, directory))]
pub async fn save_markdown(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    document: State<'_, DocumentPath>,
    content: String,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let Some(path) = pick_save_path(
        &app,
        "Save Markdown As…",
        ("Markdown", &["md"]),
        format!("{title}.md"),
        &directory,
    )?
    else {
        info!(canceled = true, "save markdown canceled");
        return Ok(FileOutcome::canceled());
    };

    let service = state.inner().clone();
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.write_document(&write_path, content.as_bytes())
    })
    .await
    .map_err(|e| format!("Save task failed: {e}"))?
    .map_err(|e| format!("Failed to save: {e}"))?;
    document.set(&path);
    // The edits are on disk: the recovery snapshot (if any) is obsolete.
    if let Some(dir) = recovery_dir(&app) {
        let _ = recovery::clear(&dir, Some(&path));
    }
    info!(path = %path.display(), "markdown saved");
    Ok(FileOutcome::saved(&path))
}

/// Save the open document in place (M9): write to the exact `PathBuf` the
/// backend remembers, with no dialog and no title-derived filename. This is
/// what makes Save-after-Open correct for a non-UTF-8 filename — the path
/// never round-trips through the lossy IPC string.
///
/// Errors when no document path is known (a new, untitled document); the
/// frontend then runs Save As instead.
#[tauri::command]
#[instrument(skip(state, document, content), fields(len = content.len()))]
pub async fn save_document(
    app: AppHandle,
    state: State<'_, Arc<Service>>,
    document: State<'_, DocumentPath>,
    content: String,
) -> Result<FileOutcome, String> {
    let Some(path) = document.get() else {
        return Err("This document has no file yet — use Save As…".into());
    };

    let service = state.inner().clone();
    let write_path = path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.write_document(&write_path, content.as_bytes())
    })
    .await
    .map_err(|e| format!("Save task failed: {e}"))?
    .map_err(|e| format!("Failed to save: {e}"))?;
    // The edits are on disk: the recovery snapshot (if any) is obsolete.
    if let Some(dir) = recovery_dir(&app) {
        let _ = recovery::clear(&dir, Some(&path));
    }
    info!(path = %path.display(), "markdown saved in place");
    Ok(FileOutcome::saved(&path))
}

/// Write (or replace) the recovery snapshot for the open document (M9 task
/// 4). Called by the frontend's autosave timer while the document is dirty;
/// the exact path comes from the backend, so non-UTF-8 filenames snapshot
/// under the right key. Best-effort: a missing app-data directory disables
/// recovery instead of failing the edit.
#[tauri::command]
#[instrument(skip(document, markdown), fields(len = markdown.len()))]
pub fn save_recovery_snapshot(
    app: AppHandle,
    document: State<'_, DocumentPath>,
    markdown: String,
) -> Result<(), String> {
    let Some(dir) = recovery_dir(&app) else {
        return Ok(());
    };
    let path = document.get();
    recovery::save(&dir, path.as_deref(), &markdown)
        .map_err(|e| format!("Failed to write the recovery snapshot: {e}"))
}

/// The newest recovery snapshot worth offering, if any (M9 task 4). Never
/// deletes the snapshot: only an explicit discard or a successful save does.
#[tauri::command]
#[instrument]
pub fn take_recovery(app: AppHandle) -> Option<RecoverySnapshot> {
    recovery_dir(&app).and_then(|dir| recovery::take(&dir))
}

/// Restore a snapshot into the editor (M9 task 4): the backend remembers the
/// exact path for save-in-place and returns the text plus the display path.
#[tauri::command]
#[instrument(skip(document), fields(id))]
pub fn restore_recovery(
    app: AppHandle,
    document: State<'_, DocumentPath>,
    id: String,
) -> Result<RestoredSnapshot, String> {
    let Some(dir) = recovery_dir(&app) else {
        return Err("Recovery is unavailable: no app-data directory".into());
    };
    let restored = recovery::restore(&dir, &id)
        .map_err(|e| format!("Failed to read the recovery snapshot: {e}"))?
        .ok_or_else(|| "The recovery snapshot is no longer available".to_string())?;
    match &restored.exact {
        Some(path) => document.set(path),
        None => document.clear(),
    }
    info!(id, "recovery snapshot restored");
    Ok(restored)
}

/// The user chose Discard in the recovery prompt (M9 task 4).
#[tauri::command]
#[instrument(fields(id))]
pub fn discard_recovery(app: AppHandle, id: String) -> Result<(), String> {
    if let Some(dir) = recovery_dir(&app) {
        recovery::discard(&dir, &id)
            .map_err(|e| format!("Failed to discard the recovery snapshot: {e}"))?;
    }
    Ok(())
}

/// Drop the snapshots for the open document and the untitled slot (M9 task
/// 4): the user chose Discard in the unsaved-changes prompt, so those edits
/// must not be offered again at the next launch.
#[tauri::command]
#[instrument(skip(document))]
pub fn clear_recovery(app: AppHandle, document: State<'_, DocumentPath>) -> Result<(), String> {
    if let Some(dir) = recovery_dir(&app) {
        let path = document.get();
        recovery::clear(&dir, path.as_deref())
            .map_err(|e| format!("Failed to clear the recovery snapshot: {e}"))?;
    }
    Ok(())
}

/// Forget the remembered document path (M9). Called by New so a later plain
/// save cannot overwrite the file that was open before; the old document's
/// recovery snapshot is dropped too, because New discards those edits.
#[tauri::command]
#[instrument(skip(app, document))]
pub fn forget_document(app: AppHandle, document: State<'_, DocumentPath>) {
    let previous = document.get();
    document.clear();
    if let Some(dir) = recovery_dir(&app) {
        let _ = recovery::clear(&dir, previous.as_deref());
    }
    debug!("document path cleared");
}

/// The working directory at startup: `$XDG_DATA_HOME` when set and non-empty,
/// else `$HOME` (SPEC §8.4).
#[tauri::command]
#[instrument]
pub fn get_initial_directory() -> String {
    // `$HOME` on POSIX, `%USERPROFILE%` on Windows; the dialog falls back to
    // the OS default when neither is set.
    let home = markdown_blathy_core::files::home_directory(
        std::env::var_os("HOME"),
        std::env::var_os("USERPROFILE"),
    );
    markdown_blathy_core::files::initial_directory(std::env::var_os("XDG_DATA_HOME"), home)
        .to_string_lossy()
        .into_owned()
}

/// Open an http(s) URL in the system default browser (§7.4, M4).
///
/// Preview links must never navigate the webview: only http(s) is forwarded
/// to the browser; every other scheme is rejected with an error (§9).
#[tauri::command]
#[instrument]
pub fn open_external(url: String) -> Result<(), String> {
    if !markdown_blathy_core::links::is_browser_url(&url) {
        return Err(format!(
            "Blocked external link: only http(s) URLs can be opened in the browser — got: {url}"
        ));
    }
    open::that(&url).map_err(|e| format!("Failed to open browser: {e}"))
}

/// Update the document's front matter with a settings patch (§6.2) and
/// return the full new Markdown. Rust owns the YAML editing: managed keys
/// (`title`/`author`/`autoHeadingLevel`/`lang`/`hyphenate`/`flavor`/`ext_*`)
/// are rewritten in canonical form and any keys this app does not manage are
/// preserved verbatim. Synchronous and CPU-trivial, so no `spawn_blocking`.
#[tauri::command]
#[instrument(skip(state, markdown, patch))]
pub fn update_front_matter(
    state: State<'_, Arc<Service>>,
    markdown: String,
    patch: Patch,
) -> Result<String, String> {
    state
        .update_front_matter(&markdown, &patch)
        .map_err(|e| e.to_string())
}

/// Validate a custom `.typ` template by compiling a probe document (§7.4, M5).
///
/// A template is accepted only when the probe compiles — a file that cannot be
/// read, or that omits/misdefines the required `article` function, is rejected
/// with the offending path and the compile diagnostics (§6.3 / §9). The UI
/// keeps the previous template (or the embedded default) on failure.
///
/// The probe engine is built exactly as the real render will use it (current
/// options + system fonts + the template's own folder, M6), and — since that
/// build already paid the font scan — the service hands it to the cache under
/// its static source. The next `render_preview` for the same template finds it
/// ready, so the document re-renders immediately instead of re-scanning.
#[tauri::command]
#[instrument(skip(state), fields(path, options))]
pub async fn validate_template(
    state: State<'_, Arc<Service>>,
    path: String,
    options: RenderOptions,
) -> Result<serde_json::Value, String> {
    let service = state.inner().clone();
    let template_path = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || {
        service.validate_template(&template_path, &options)
    })
    .await
    .map_err(|e| format!("Validation task failed: {e}"))?
    .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "valid": true }))
}

/// List the bundled template ids (one subdirectory per template, each with a
/// `main.typ`), ordered with the default `light` first. The frontend maps the
/// ids to display labels and shows them in the template selector (§8.6).
#[tauri::command]
#[instrument(skip(state))]
pub fn list_templates(state: State<'_, Arc<Service>>) -> Result<Vec<String>, String> {
    state.list_templates().map_err(|e| e.to_string())
}

/// Open a file in the platform's default viewer (M4 acceptance: "Export
/// writes a PDF that opens in a viewer"). Non-fatal: the export already
/// succeeded on disk even if no viewer can be launched.
fn open_in_viewer(path: &Path) {
    if let Err(err) = open::that(path) {
        tracing::warn!(error = %err, "failed to open PDF in viewer");
    }
}

#[cfg(test)]
mod tests {
    use super::{DocMeta, FileOutcome, PreviewOutcome, RenderPayload};
    use markdown_blathy_core::options::{FeatureFlags, Flavor, Margins, Paper, TextAlign};

    /// Pins the IPC contract: the exact JSON the frontend `lib/api.js` sends
    /// (§8.4) must deserialize into `RenderPayload`.
    #[test]
    fn render_payload_deserializes_frontend_shape() {
        let json = r##"{
            "markdown": "# Hi",
            "options": {
                "flavor": "github",
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
        assert_eq!(payload.options.flavor, Flavor::Github);
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
    /// camelCase shape the frontend reads (§7.4), including the effective
    /// document settings (§6.2).
    #[test]
    fn preview_outcome_serializes_with_camel_case_and_seq() {
        let outcome = PreviewOutcome {
            pages: vec!["<svg>".into()],
            warnings: vec!["font warning".into()],
            doc: DocMeta {
                title: Some("My Doc".into()),
                author: None,
                lang: Some("hu".into()),
                hyphenate: Some(false),
                flavor: Flavor::Github,
                features: FeatureFlags::default(),
                auto_heading_level: 6,
                toc: vec![],
            },
            seq: 3,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("\"pages\":[\"<svg>\"]"), "{json}");
        assert!(json.contains("\"warnings\":[\"font warning\"]"), "{json}");
        assert!(json.contains("\"seq\":3"), "{json}");
        assert!(json.contains("\"doc\":{"), "{json}");
        assert!(json.contains("\"title\":\"My Doc\""), "{json}");
        assert!(json.contains("\"flavor\":\"github\""), "{json}");
        assert!(json.contains("\"autoHeadingLevel\":6"), "{json}");
        // The Document settings widget reads these back (§6.2).
        assert!(json.contains("\"lang\":\"hu\""), "{json}");
        assert!(json.contains("\"hyphenate\":false"), "{json}");
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
