//! Thin `#[tauri::command]` wrappers (§7.4 of SPEC.md).
//!
//! All real logic lives in the plain modules (`markdown/`, `typst_engine/`,
//! `files/`) which have no `tauri` imports; these functions are
//! serialization-only shims plus the system dialogs.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};

use crate::files;
use crate::options::RenderOptions;
use crate::typst_engine::engine::{self, FontOptions};
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

/// The M6 engine cache (§7.5): one compiled engine per static source.
///
/// Rebuilding an engine is the expensive part — the font scan runs at build
/// time — so the engine is reused across renders and recreated only when the
/// static source (the resolved template plus the toolbar options) changes.
/// The markdown body never touches it: it travels as `sys.inputs` per render.
pub struct TemplateEngineCache {
    key: String,
    engine: Option<TypstEngine<TypstTemplateMainFile>>,
}

impl TemplateEngineCache {
    pub fn new() -> Self {
        Self {
            key: String::new(),
            engine: None,
        }
    }

    /// The engine for `static_source`, rebuilt (fonts re-scanned) only when
    /// the source changed since the last call.
    fn engine_for(
        &mut self,
        static_source: &str,
        fonts: &FontOptions,
    ) -> Result<&TypstEngine<TypstTemplateMainFile>, String> {
        if self.key != static_source {
            // Diagnostic (stderr): a rebuild re-runs the font scan, which is
            // the ~1.5s cost the user may see on the first render after a
            // template change. Each line here should correspond to an actual
            // template/options change, never to steady-state typing.
            eprintln!(
                "[engine cache] rebuild: static source changed ({} -> {} chars)",
                self.key.len(),
                static_source.len()
            );
            self.engine =
                Some(engine::build_engine(static_source, fonts).map_err(|e| e.to_string())?);
            self.key = static_source.to_string();
        }
        Ok(self.engine.as_ref().expect("engine built just above"))
    }

    /// Store a ready-built engine under its static source. Used by template
    /// validation (§7.4): validation already scans the fonts and builds an
    /// engine, so it hands it to the cache and the next render — which uses
    /// the same static source — finds it ready instead of re-scanning.
    fn prime(&mut self, static_source: &str, engine: TypstEngine<TypstTemplateMainFile>) {
        self.key = static_source.to_string();
        self.engine = Some(engine);
    }
}

impl Default for TemplateEngineCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Font loading for a render (§6.3, M6): the embedded DejaVu Sans always
/// applies. A custom template loads only the font families it references
/// (`#set text(font: ...)`) — a small font book keeps compiles fast — plus
/// any font files next to the `.typ` file. Templates that reference fonts
/// via variables (not detected by the parser) fall back to the full system
/// scan.
fn font_options_for(options: &RenderOptions) -> FontOptions {
    match &options.template_path {
        Some(path) => font_options_for_path(path),
        None => FontOptions::default(),
    }
}

/// Font loading for a template *file path* (used by validation, which runs
/// before the template is applied to the options).
fn font_options_for_path(path: &str) -> FontOptions {
    let dir = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf());
    let families = std::fs::read_to_string(path)
        .map(|source| template::referenced_font_families(&source))
        .unwrap_or_default();
    FontOptions {
        // When no families are detected, keep the conservative full scan.
        system_fonts: families.is_empty(),
        extra_dirs: dir.into_iter().collect(),
        families,
    }
}

/// Live preview: markdown + options → per-page SVG strings (§7.4).
///
/// The static source (template + toolbar options) selects a cached engine —
/// built once, fonts scanned once — and the markdown body is passed per
/// render via `sys.inputs` (M6, §7.5). Compilation is CPU-bound, so it runs
/// on `spawn_blocking`; `seq` is echoed back so the frontend can drop stale
/// responses.
#[tauri::command]
pub async fn render_preview(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    payload: RenderPayload,
) -> Result<PreviewOutcome, String> {
    let t0 = std::time::Instant::now();
    let template_source = resolve_template_source(&payload.options)?;
    let static_source = template::build_static_source(&template_source, &payload.options)?;
    let inputs = template::render_inputs(&payload.markdown, &payload.options)?;
    let fonts = font_options_for(&payload.options);
    let t1 = std::time::Instant::now();

    let cache = state.inner().clone(); // owned Arc<Mutex<...>> for the blocking thread
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache
            .lock()
            .map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = cache.engine_for(&static_source, &fonts)?;
        engine::compile_preview_with_inputs(engine, &inputs).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Preview task failed: {e}"))??;

    // Diagnostic: where does the per-render time go? `setup` is the frontend
    // payload handling (template read + conversion), `render` is the cached
    // engine compile + SVG generation. If `render` is small but the user still
    // sees ~1s, the cost is on the frontend (IPC/DOM), not the backend.
    let t2 = std::time::Instant::now();
    let svg_bytes: usize = outcome.pages.iter().map(|p| p.len()).sum();
    eprintln!(
        "[preview] setup={:?} render={:?} total={:?} pages={} svg={svg_bytes}B",
        t1.duration_since(t0),
        t2.duration_since(t1),
        t2.duration_since(t0),
        outcome.pages.len(),
    );

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
/// Shares the M6 engine cache with the preview, so exporting does not re-scan
/// fonts. Compilation and the blocking save dialog run off the UI thread.
#[tauri::command]
pub async fn export_pdf(
    app: AppHandle,
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    payload: RenderPayload,
    title: String,
    directory: Option<String>,
) -> Result<FileOutcome, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let static_source = template::build_static_source(&template_source, &payload.options)?;
    let inputs = template::render_inputs(&payload.markdown, &payload.options)?;
    let fonts = font_options_for(&payload.options);

    let cache = state.inner().clone(); // owned Arc<Mutex<...>> for the blocking thread
    let pdf = tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache
            .lock()
            .map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = cache.engine_for(&static_source, &fonts)?;
        engine::compile_pdf_with_inputs(engine, &inputs).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Export task failed: {e}"))??;

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
pub async fn validate_template(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    path: String,
    options: RenderOptions,
) -> Result<serde_json::Value, String> {
    let user_source = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read template at {path}: {e}"))?;
    let template_source = format!("{}\n\n{user_source}", template::DEFAULT_TEMPLATE);
    let static_source = template::build_static_source(&template_source, &options)?;
    let fonts = font_options_for_path(&path);

    let cache = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache
            .lock()
            .map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = engine::build_engine(&static_source, &fonts).map_err(|e| e.to_string())?;
        engine::compile_preview_with_inputs(&engine, &template::probe_inputs())
            .map_err(|e| e.to_string())?;
        cache.prime(&static_source, engine);
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("Validation task failed: {e}"))?
    .map_err(|e| format!("Template at {path} rejected: {e}"))?;
    Ok(serde_json::json!({ "valid": true }))
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
    use super::{FileOutcome, PreviewOutcome, RenderPayload, TemplateEngineCache};
    use crate::options::{Flavor, Margins, Paper, TextAlign};
    use crate::typst_engine::engine::{self, FontOptions};

    /// M6: the engine cache serves the engine for the current static source
    /// and rebuilds it (new main-file content) as soon as the source changes —
    /// the per-template font scan therefore runs once per source change.
    #[test]
    fn engine_cache_serves_and_rebuilds_on_source_change() {
        use typst::World;

        let mut cache = TemplateEngineCache::new();
        let fonts = FontOptions::default();

        fn main_source(
            engine: &typst_as_lib::TypstEngine<typst_as_lib::TypstTemplateMainFile>,
        ) -> String {
            engine
                .with_world(|world| {
                    world
                        .source(world.main())
                        .map(|s| s.text().to_owned())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        }

        let engine = cache.engine_for("source one", &fonts).expect("must build");
        assert_eq!(main_source(engine), "source one");

        let engine = cache.engine_for("source one", &fonts).expect("must reuse");
        assert_eq!(
            main_source(engine),
            "source one",
            "reused engine keeps its source"
        );

        let engine = cache
            .engine_for("source two", &fonts)
            .expect("must rebuild");
        assert_eq!(
            main_source(engine),
            "source two",
            "changed static source must rebuild the engine"
        );
    }

    /// M6: validation primes the cache with a ready-built engine (§7.4); a
    /// subsequent render for the same static source is served that engine
    /// instead of re-scanning fonts.
    #[test]
    fn cache_prime_serves_the_prebuilt_engine() {
        use typst::World;

        let mut cache = TemplateEngineCache::new();
        let fonts = FontOptions::default();

        fn main_source(
            engine: &typst_as_lib::TypstEngine<typst_as_lib::TypstTemplateMainFile>,
        ) -> String {
            engine
                .with_world(|world| {
                    world
                        .source(world.main())
                        .map(|s| s.text().to_owned())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        }

        let prebuilt = engine::build_engine("theme source", &fonts).expect("must build");
        cache.prime("theme source", prebuilt);
        let served = cache
            .engine_for("theme source", &fonts)
            .expect("must reuse");
        assert_eq!(
            main_source(served),
            "theme source",
            "the primed engine must be served without a rebuild"
        );
    }

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
