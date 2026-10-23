//! Thin `#[tauri::command]` wrappers (§7.4 of SPEC.md).
//!
//! All real logic lives in the plain modules (`markdown/`, `typst_engine/`,
//! `files/`) which have no `tauri` imports; these functions are
//! serialization-only shims plus the system dialogs.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tracing::{debug, info, instrument, warn};
use typst::foundations::Dict;
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};

use crate::engine_cache::TemplateEngineCache;
use crate::files;
use crate::markdown::convert::DocMeta;
use crate::options::RenderOptions;
use crate::typst_engine::engine;
use crate::typst_engine::template;

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

/// The directory holding the bundled templates (one subdirectory per
/// template, each with a `main.typ`). Resolved from the Tauri resource dir;
/// in dev (`cargo run`/`tauri dev`) the resources are not bundled, so fall
/// back to the in-tree `src-tauri/templates/` next to the manifest.
fn templates_dir(app: &AppHandle) -> Option<PathBuf> {
    use tauri::Manager as _;
    crate::files::templates_dir(app.path().resource_dir().ok())
}

/// The UI shows converter warnings (e.g. duplicate heading IDs) and engine
/// warnings (e.g. missing fonts) together (§3): converter first, engine after.
fn merged_warnings(mut converter: Vec<String>, engine: Vec<String>) -> Vec<String> {
    converter.extend(engine);
    converter
}

/// Run `work` against the cached engine for a prepared render (§7.5, M6).
///
/// Compilation is CPU-bound, so the cache lock, the engine lookup/rebuild and
/// the compile itself run on `spawn_blocking`; the shared `(static_source,
/// fonts, inputs)` triple is cloned in so the closure is `'static`. Shared by
/// `render_preview` and `export_pdf`, which differ only in `work`.
async fn with_cached_engine<R: Send + 'static>(
    state: &State<'_, Arc<Mutex<TemplateEngineCache>>>,
    prepared: &template::PreparedRender,
    task: &'static str,
    work: impl FnOnce(&TypstEngine<TypstTemplateMainFile>, &Dict) -> Result<R, String> + Send + 'static,
) -> Result<R, String> {
    let cache = state.inner().clone(); // owned Arc<Mutex<...>> for the blocking thread
    let static_source = prepared.static_source.clone();
    let fonts = prepared.fonts.clone();
    let inputs = prepared.inputs.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache
            .lock()
            .map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = cache.engine_for(&static_source, &fonts)?;
        work(engine, &inputs)
    })
    .await
    .map_err(|e| format!("{task} failed: {e}"))?
}

/// Live preview: markdown + options → per-page SVG strings (§7.4).
///
/// The static source (template + toolbar options) selects a cached engine —
/// built once, fonts scanned once — and the markdown body is passed per
/// render via `sys.inputs` (M6, §7.5). Compilation is CPU-bound, so it runs
/// on `spawn_blocking`; `seq` is echoed back so the frontend can drop stale
/// responses.
#[tauri::command]
#[instrument(skip(state, app, payload), fields(seq = payload.seq, md_len = payload.markdown.len()))]
pub async fn render_preview(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    app: AppHandle,
    payload: RenderPayload,
) -> Result<PreviewOutcome, String> {
    let t0 = std::time::Instant::now();
    let prepared = template::prepare_render(
        &payload.markdown,
        &payload.options,
        templates_dir(&app).as_deref(),
        payload.document_path.as_deref().map(Path::new),
    )?;
    let t1 = std::time::Instant::now();

    let outcome = with_cached_engine(&state, &prepared, "Preview task", |engine, inputs| {
        engine::compile_preview_with_inputs(engine, inputs).map_err(|e| e.to_string())
    })
    .await?;

    let t2 = std::time::Instant::now();
    let svg_bytes: usize = outcome.pages.iter().map(|p| p.len()).sum();

    // Timing breakdown at debug level (RUST_LOG=debug) — `setup` is the
    // frontend payload handling, `render` is the cached engine compile + SVG
    // generation. If `render` is small but the user still sees ~1s, the cost
    // is on the frontend (IPC/DOM), not the backend.
    debug!(
        setup_ms = t1.duration_since(t0).as_millis(),
        render_ms = t2.duration_since(t1).as_millis(),
        total_ms = t2.duration_since(t0).as_millis(),
        "render timing"
    );

    // Summary at info level — return value contains full SVG pages (too large
    // for `#[instrument] ret`).
    info!(pages = outcome.pages.len(), svg_bytes, "preview rendered");

    // Prewarm hint for the next launch (§7.5): the static source just
    // rendered with is what the next launch's first render will want, so
    // setup() can build that engine under the boot loader. Saved on a
    // background thread, and a no-op while the file already holds it.
    let hint = crate::prewarm::PrewarmHint {
        static_source: prepared.static_source.clone(),
        fonts: prepared.fonts.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager as _;
        let Ok(dir) = app.path().app_data_dir() else {
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = crate::prewarm::save_hint(&crate::prewarm::hint_path(&dir), &hint) {
            debug!("prewarm hint not saved: {e}");
        }
    });

    Ok(PreviewOutcome {
        pages: outcome.pages,
        warnings: merged_warnings(prepared.warnings.clone(), outcome.warnings),
        doc: prepared.doc.clone(),
        seq: payload.seq,
    })
}

/// Export: markdown + options → system save dialog (defaulting to `<title>.pdf`
/// in `directory`) → write the PDF atomically → open it in the system viewer
/// (§7.4, M4).
///
/// Shares the M6 engine cache with the preview, so exporting does not re-scan
/// fonts. Compilation and the blocking save dialog run off the UI thread.
#[tauri::command]
#[instrument(skip(app, state, payload), fields(title, directory))]
pub async fn export_pdf(
    app: AppHandle,
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    payload: RenderPayload,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let prepared = template::prepare_render(
        &payload.markdown,
        &payload.options,
        templates_dir(&app).as_deref(),
        payload.document_path.as_deref().map(Path::new),
    )?;

    // `FileOutcome` has no warnings channel (§8.4), so converter warnings
    // are logged rather than surfaced in the UI on export.
    if !prepared.warnings.is_empty() {
        warn!(warnings = ?prepared.warnings, "converter warnings during export");
    }

    let pdf = with_cached_engine(&state, &prepared, "Export task", |engine, inputs| {
        engine::compile_pdf_with_inputs(engine, inputs).map_err(|e| e.to_string())
    })
    .await?;

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

    files::write_atomically(&path, &pdf).map_err(|e| format!("Failed to write PDF: {e}"))?;
    open_in_viewer(&path);
    info!(path = %path.display(), "export PDF saved");
    Ok(FileOutcome::saved(&path))
}

/// A system *open* dialog with the given filters (name + extensions),
/// starting in `directory` when given. `None` means the user dismissed it —
/// the caller treats that as silent (§9).
fn pick_open_path(
    app: &AppHandle,
    title: &str,
    filters: &[(&str, &[&str])],
    directory: &Option<String>,
) -> Result<Option<std::path::PathBuf>, String> {
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
fn pick_save_path(
    app: &AppHandle,
    title: &str,
    filter: (&str, &[&str]),
    file_name: String,
    directory: &Option<String>,
) -> Result<Option<std::path::PathBuf>, String> {
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

/// Open a Markdown file via the system dialog (starting in `directory` when
/// given) and read it (§7.4, M4).
#[tauri::command]
#[instrument(skip(app), fields(directory))]
pub async fn open_markdown(
    app: AppHandle,
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

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {e}"))?;
    info!(path = %path.display(), "markdown opened");
    Ok(FileOutcome::opened(&path, content))
}

/// Save Markdown via a "Save As…" dialog: the filename defaults to
/// `<title>.md` in `directory`, and the write is atomic (§7.4, M4).
#[tauri::command]
#[instrument(skip(app, content), fields(title, directory))]
pub async fn save_markdown(
    app: AppHandle,
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

    files::write_atomically(&path, content.as_bytes())
        .map_err(|e| format!("Failed to save: {e}"))?;
    info!(path = %path.display(), "markdown saved");
    Ok(FileOutcome::saved(&path))
}

/// The working directory at startup: `$XDG_DATA_HOME` when set and non-empty,
/// else `$HOME` (SPEC §8.4).
#[tauri::command]
#[instrument]
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
#[instrument]
pub fn open_external(url: String) -> Result<(), String> {
    if !crate::links::is_browser_url(&url) {
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
#[instrument(skip(markdown))]
pub fn update_front_matter(markdown: String, patch: crate::markdown::frontmatter::Patch) -> String {
    crate::markdown::frontmatter::apply_patch(&markdown, &patch)
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
/// build already paid the font scan — the engine is handed to the cache under
/// its static source. The next `render_preview` for the same template finds it
/// ready, so the document re-renders immediately instead of re-scanning.
#[tauri::command]
#[instrument(skip(state), fields(path, options))]
pub async fn validate_template(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    path: String,
    options: RenderOptions,
) -> Result<serde_json::Value, String> {
    let user_source = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read template at {path}: {e}"))?;
    let template_source = format!("{}\n\n{user_source}", template::DEFAULT_TEMPLATE);
    let static_source = template::build_static_source(&template_source, &options)?;
    // Font scan runs on the same bytes already read above — one file read.
    let fonts = engine::font_options_for_source(&path, &user_source);

    let cache = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache
            .lock()
            .map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = template::build_and_probe(&static_source, &fonts)?;
        cache.prime(&static_source, &fonts, engine);
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Validation task failed: {e}"))?
    .map_err(|e| format!("Template at {path} rejected: {e}"))?;
    Ok(serde_json::json!({ "valid": true }))
}

/// List the bundled template ids (one subdirectory per template, each with a
/// `main.typ`), ordered with the default `light` first. The frontend maps the
/// ids to display labels and shows them in the template selector (§8.6).
#[tauri::command]
#[instrument]
pub fn list_templates(app: AppHandle) -> Result<Vec<String>, String> {
    let dir =
        templates_dir(&app).ok_or_else(|| "Bundled templates are not available".to_string())?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .map_err(|e| format!("Failed to list templates at {}: {e}", dir.display()))?
    {
        let entry = entry.map_err(|e| format!("Failed to read template entry: {e}"))?;
        if entry.path().join("main.typ").is_file() {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    // `light` is the default, so it leads the selector regardless of sort order.
    if let Some(pos) = names.iter().position(|n| n == "light") {
        let light = names.remove(pos);
        names.insert(0, light);
    }
    Ok(names)
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
        tracing::warn!(error = %err, "failed to open PDF in viewer");
    }
}

#[cfg(test)]
mod tests {
    use super::{DocMeta, FileOutcome, PreviewOutcome, RenderPayload};
    use crate::options::{FeatureFlags, Flavor, Margins, Paper, TextAlign};

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

    /// Converter warnings come first, engine warnings after — the order the
    /// StatusBar lists them in (§3, §9).
    #[test]
    fn merged_warnings_chains_converter_then_engine() {
        let merged = super::merged_warnings(
            vec!["duplicate heading ID".into()],
            vec!["font warning".into()],
        );
        assert_eq!(
            merged,
            vec![
                "duplicate heading ID".to_string(),
                "font warning".to_string()
            ]
        );

        // Either side may be empty.
        assert_eq!(
            super::merged_warnings(Vec::new(), vec!["engine".into()]),
            vec!["engine".to_string()]
        );
        assert_eq!(
            super::merged_warnings(vec!["converter".into()], Vec::new()),
            vec!["converter".to_string()]
        );
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
