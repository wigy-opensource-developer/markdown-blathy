//! Typst compile & export (§6.4 of SPEC.md).
//!
//! Uses `typst-as-lib` 0.16 so we never hand-write a `typst::World`. Pure Rust
//! — no `tauri` imports. Two entry points, both compiling the engine's static
//! source with per-render `sys.inputs` (M6, §7.5):
//!
//! - `build_engine` + `compile_preview_with_inputs`: SVG pages for the live
//!   preview pane. The command layer caches the engine and rebuilds it (and
//!   re-runs the font scan) only when the static source changes.
//! - `build_engine` + `compile_pdf_with_inputs`: PDF bytes for export.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use comemo::Track;
use typst::diag::EcoString;
use typst::foundations::Dict;
use typst::layout::{FrameItem, Point};
use typst::model::{Destination, LateLinkResolver, ResolvedLink};
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};
use typst_layout::PagedDocument;

/// Embedded default fonts (DejaVu, permissive license — §6.3). The mono
/// face is required by `raw()`: the default template's font book loads no
/// system fonts, so without an embedded monospace face code blocks would
/// silently fall back to the proportional body font.
pub const FONT_BYTES: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
pub const FONT_BOLD_BYTES: &[u8] = include_bytes!("../../fonts/DejaVuSans-Bold.ttf");
pub const FONT_MONO_BYTES: &[u8] = include_bytes!("../../fonts/DejaVuSansMono.ttf");

/// Per-page SVG strings plus any compile warnings (never fatal).
#[derive(Debug)]
pub struct PreviewOutcome {
    pub pages: Vec<String>,
    pub warnings: Vec<String>,
}

/// Compilation failures, rendered as user-presentable messages (§9).
#[derive(Debug)]
pub enum EngineError {
    /// The Typst source did not compile; carries the rendered diagnostics.
    Compile(String),
    /// PDF export failed.
    Pdf(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Compile(msg) => write!(f, "Preview failed: {msg}"),
            EngineError::Pdf(msg) => write!(f, "PDF export failed: {msg}"),
        }
    }
}

impl Error for EngineError {}

/// Font loading options (M6, §6.3): what to load beyond the embedded DejaVu
/// Sans. Used when an engine is (re)built — the M6 cache does that once per
/// template change, so the system-font scan is not repeated per render.
#[derive(Debug, Clone, Default)]
pub struct FontOptions {
    /// Scan the system's font directories (needed for families like
    /// "Times New Roman" in custom templates). Ignored when `families` is set.
    pub system_fonts: bool,
    /// Extra directories to scan recursively (e.g. the folder next to a
    /// custom template, so a theme can ship its own font files).
    pub extra_dirs: Vec<PathBuf>,
    /// When non-empty, only these families are loaded from the system (plus
    /// the embedded DejaVu Sans and the `extra_dirs`). A small font book keeps
    /// per-render compiles fast — a 700-face book makes them ~5× slower.
    pub families: Vec<String>,
}

/// Font loading for a template whose source has already been read. Used by
/// `template::prepare_render` / validation so the template file is read exactly
/// once — the same bytes feed both the composed source and the font-family
/// scan.
pub fn font_options_for_source(path: &str, source: &str) -> FontOptions {
    let dir = Path::new(path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf());
    let families = crate::typst_engine::template::referenced_font_families(source);
    FontOptions {
        // When no families are detected, keep the conservative full scan.
        system_fonts: families.is_empty(),
        extra_dirs: dir.into_iter().collect(),
        families,
    }
}

/// A cached scan of the system's fonts: `(path, family)`. The full scan is
/// ~1.4s on font-heavy machines, so it runs once per process and is reused
/// across template switches (§6.3).
fn system_fonts() -> &'static Vec<(PathBuf, String)> {
    static CACHE: std::sync::OnceLock<Vec<(PathBuf, String)>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| {
        typst_kit::fonts::system()
            .map(|(path, info)| (path.path, info.family))
            .collect()
    })
}

/// Read the font files for the given families, from the system scan and any
/// extra directories, deduplicated by path.
fn load_families(families: &[String], extra_dirs: &[PathBuf]) -> Vec<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut push = |path: &PathBuf, family: &str| {
        if families.iter().any(|f| f.eq_ignore_ascii_case(family)) && seen.insert(path.clone()) {
            if let Ok(data) = std::fs::read(path) {
                bytes.push(data);
            }
        }
    };
    for (path, family) in system_fonts() {
        push(path, family);
    }
    for dir in extra_dirs {
        for (path, info) in typst_kit::fonts::scan(dir) {
            push(&path.path, &info.family);
        }
    }
    bytes
}

/// Build a Typst engine for the given static source (§6.4, M6).
///
/// The embedded DejaVu Sans is always available. When the template references
/// specific font families (`FontOptions.families`), only those are loaded —
/// a small font book keeps per-render compiles fast; otherwise `fonts` adds
/// the full system scan and/or extra directories. Called by the command
/// layer's engine cache, which rebuilds (and re-runs the font scan) only when
/// the static source changes.
pub fn build_engine(
    source: &str,
    fonts: &FontOptions,
) -> Result<TypstEngine<TypstTemplateMainFile>, EngineError> {
    let mut all_fonts: Vec<Vec<u8>> = vec![
        FONT_BYTES.to_vec(),
        FONT_BOLD_BYTES.to_vec(),
        FONT_MONO_BYTES.to_vec(),
    ];
    let mut search_system = false;
    if !fonts.families.is_empty() {
        // Fast path: only the referenced families + embedded + extra dirs.
        all_fonts.extend(load_families(&fonts.families, &fonts.extra_dirs));
    } else if fonts.system_fonts || !fonts.extra_dirs.is_empty() {
        search_system = true;
    }
    let mut builder = TypstEngine::builder().main_file(source).fonts(all_fonts);
    if search_system {
        let mut options = typst_as_lib::typst_kit_options::TypstKitFontOptions::default();
        if !fonts.system_fonts {
            options = options.include_system_fonts(false);
        }
        if !fonts.extra_dirs.is_empty() {
            options = options.include_dirs(fonts.extra_dirs.iter().cloned());
        }
        builder = builder.search_fonts_with(options);
    }
    // typst-as-lib's builder default (`Some(0)`) evicts the whole comemo
    // memoization cache after every compile — re-reading font files and
    // re-doing shared work on each keystroke render (~7x slower with system
    // fonts). Keep entries warm across renders but bound memory: entries not
    // touched for this many compiles are evicted (§7.5, M6).
    builder.comemo_evict_max_age(Some(30));
    Ok(builder.build())
}

/// Compile the engine's static source with the per-render `sys.inputs` and
/// render preview pages as SVG (§6.4).
///
/// Uses `svg_in_bundle` with a `LateLinkResolver` so internal links —
/// footnote markers ↔ notes, heading anchors, cross-references — resolve to
/// clickable `<a>` elements in the SVG. The plain `typst_svg::svg()` omits
/// `href` on every internal link.
pub fn compile_preview_with_inputs(
    engine: &TypstEngine<TypstTemplateMainFile>,
    inputs: &Dict,
) -> Result<PreviewOutcome, EngineError> {
    let warned = engine.compile_with_input(inputs.clone());
    let doc: PagedDocument = warned
        .output
        .map_err(|e| EngineError::Compile(render_error(engine, &e)))?;
    let warnings = warned
        .warnings
        .iter()
        .map(|diag| render_diagnostic(engine, diag))
        .collect();

    let pages = compile_preview_pages(&doc);
    Ok(PreviewOutcome { pages, warnings })
}

/// Render all pages as SVG with internal link resolution (footnotes, heading
/// anchors, cross-references).
///
/// Walks every page's frame for `FrameItem::Link(Destination::Location, …)`,
/// resolves each location via the `LateLinkResolver` to get the anchor string,
/// then calls `svg_in_bundle` with the collected anchors and a tracked
/// resolver.
fn compile_preview_pages(doc: &PagedDocument) -> Vec<String> {
    let resolver = LateLinkResolver::new(None, doc.introspector().as_ref());
    let tracked = resolver.track();

    // Collect (position, anchor_id) pairs per page.
    let mut anchors_by_page: HashMap<usize, Vec<(Point, EcoString)>> = HashMap::new();
    for page in doc.pages().iter() {
        for (_, item) in page.frame.items() {
            if let FrameItem::Link(Destination::Location(loc), _) = item {
                if let Some(pos) = doc.introspector().position(*loc) {
                    if let Some(ResolvedLink::Local { anchor }) = tracked.resolve(*loc) {
                        anchors_by_page
                            .entry(pos.page.get())
                            .or_default()
                            .push((pos.point, anchor));
                    }
                }
            }
        }
    }

    doc.pages()
        .iter()
        .enumerate()
        .map(|(page_idx, page)| {
            let anchors = anchors_by_page.get(&page_idx).cloned().unwrap_or_default();
            typst_svg::svg_in_bundle(page, &Default::default(), &anchors, tracked)
        })
        .collect()
}

/// Compile the engine's static source with the per-render `sys.inputs` and
/// export PDF bytes (§6.4).
pub fn compile_pdf_with_inputs(
    engine: &TypstEngine<TypstTemplateMainFile>,
    inputs: &Dict,
) -> Result<Vec<u8>, EngineError> {
    let warned = engine.compile_with_input(inputs.clone());
    let doc: PagedDocument = warned
        .output
        .map_err(|e| EngineError::Compile(render_error(engine, &e)))?;
    typst_pdf::pdf(&doc, &Default::default()).map_err(|e| EngineError::Pdf(format!("{e:?}")))
}

/// Map a `TypstAsLibError` to a "line N: message" string (§6.4). The first
/// ~5 diagnostics are shown; the rest are elided.
fn render_error(
    engine: &TypstEngine<TypstTemplateMainFile>,
    error: &typst_as_lib::TypstAsLibError,
) -> String {
    match error {
        typst_as_lib::TypstAsLibError::TypstSource(diags) => {
            let mut lines: Vec<String> = Vec::new();
            let _ = engine.with_world(|world| {
                for diag in diags.iter().take(5) {
                    lines.push(render_diagnostic_with_world(world, diag));
                }
            });
            if lines.is_empty() {
                "compilation failed".into()
            } else {
                lines.join("\n")
            }
        }
        other => format!("compilation failed: {other}"),
    }
}

/// Render a warning diagnostic to "line N: message" (or just the message when
/// no world can be built, e.g. file errors).
fn render_diagnostic(
    engine: &TypstEngine<TypstTemplateMainFile>,
    diag: &typst::diag::SourceDiagnostic,
) -> String {
    engine
        .with_world(|world| render_diagnostic_with_world(world, diag))
        .unwrap_or_else(|_| diag.message.to_string())
}

fn render_diagnostic_with_world(
    world: &typst_as_lib::TypstWorld<'_>,
    diag: &typst::diag::SourceDiagnostic,
) -> String {
    use typst::{World, WorldExt};
    // The range is resolved against the span's own file, so look the line up
    // in that same file's source (in our single-main-file setup it is always
    // the main file, but this stays correct if that ever changes).
    let line = world.range(diag.span).and_then(|range| {
        let id = diag.span.id()?;
        let source = world.source(id).ok()?;
        source.lines().byte_to_line(range.start).map(|l| l + 1)
    });
    match line {
        Some(n) => format!("line {n}: {}", diag.message),
        None => diag.message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_engine, compile_pdf_with_inputs, compile_preview_with_inputs, FontOptions,
        PreviewOutcome,
    };
    use crate::options::{Flavor, HeadingIds, Margins, Paper, RenderOptions, TextAlign};
    use crate::typst_engine::template::{
        build_static_source, compose, render_inputs, TemplateArgs, DEFAULT_TEMPLATE,
    };
    use typst::foundations::{Dict, IntoValue, Value};
    use typst_as_lib::{TypstEngine, TypstTemplateMainFile};

    fn gfm_options() -> RenderOptions {
        RenderOptions {
            flavor: Flavor::Gfm,
            paper: Paper::A4,
            margins: Margins {
                top_mm: 25.0,
                bottom_mm: 25.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            text_align: TextAlign::Left,
            template_path: None,
            heading_ids: HeadingIds::default(),
        }
    }

    fn custom_options() -> RenderOptions {
        RenderOptions {
            flavor: Flavor::Custom,
            ..gfm_options()
        }
    }

    /// A ready engine for the default template whose `sys.inputs.body` is
    /// `body` — the M6 shape: static source (built once per template) plus
    /// per-render inputs.
    fn engine_for_body(body: &str) -> (TypstEngine<TypstTemplateMainFile>, Dict) {
        let source = compose(
            DEFAULT_TEMPLATE,
            &TemplateArgs {
                paper: "a4",
                margin_x_mm: 20.0,
                margin_y_mm: 25.0,
                text_align: "left",
            },
        );
        let mut inputs = Dict::new();
        inputs.insert("title".into(), Value::None);
        inputs.insert("author".into(), Value::None);
        inputs.insert("body".into(), body.into_value());
        (
            build_engine(&source, &FontOptions::default()).expect("engine must build"),
            inputs,
        )
    }

    fn compile_body(body: &str) -> PreviewOutcome {
        let (engine, inputs) = engine_for_body(body);
        compile_preview_with_inputs(&engine, &inputs).expect("must compile")
    }

    fn engine_for_body_with_fonts(
        body: &str,
        fonts: &FontOptions,
    ) -> (TypstEngine<TypstTemplateMainFile>, Dict) {
        let source = compose(
            DEFAULT_TEMPLATE,
            &TemplateArgs {
                paper: "a4",
                margin_x_mm: 20.0,
                margin_y_mm: 25.0,
                text_align: "left",
            },
        );
        let mut inputs = Dict::new();
        inputs.insert("title".into(), Value::None);
        inputs.insert("author".into(), Value::None);
        inputs.insert("body".into(), body.into_value());
        (
            build_engine(&source, fonts).expect("engine must build"),
            inputs,
        )
    }

    fn compile_pdf_body(body: &str) -> Vec<u8> {
        let (engine, inputs) = engine_for_body(body);
        compile_pdf_with_inputs(&engine, &inputs).expect("must compile")
    }

    /// Full pipeline: markdown → converter → static source + inputs → compile.
    fn compile_markdown(markdown: &str, options: &RenderOptions) -> PreviewOutcome {
        let static_source = build_static_source(DEFAULT_TEMPLATE, options).unwrap();
        let (inputs, _warnings) = render_inputs(markdown, options).unwrap();
        let engine = build_engine(&static_source, &FontOptions::default()).expect("engine");
        compile_preview_with_inputs(&engine, &inputs).expect("must compile")
    }

    /// Compile markdown through a *custom* template source (prelude + user
    /// file) with the Custom flavor — the M6 flow used by the theme tests.
    fn compile_custom_theme(template: &str, markdown: &str) -> PreviewOutcome {
        let static_source = build_static_source(template, &custom_options()).unwrap();
        let (inputs, _warnings) = render_inputs(markdown, &custom_options()).unwrap();
        let engine = build_engine(&static_source, &FontOptions::default()).expect("engine");
        compile_preview_with_inputs(&engine, &inputs).expect("must compile")
    }

    /// The leftmost glyph x (absolute) in a page SVG. For a left-aligned list
    /// this is the left margin; for a centered, shrink-wrapped list it sits
    /// well inside the page. Walks the `<g transform="…">` nesting so a glyph
    /// inside `align(center, block(width: auto)[…])` reports its true page
    /// position (translate offsets + matrix offset + run offset).
    fn min_glyph_x(svg: &str) -> f64 {
        let mut stack: Vec<f64> = Vec::new();
        let mut min_x = f64::MAX;
        let mut pos = 0;
        while pos < svg.len() {
            let open = svg[pos..].find("<g");
            let close = svg[pos..].find("</g");
            let next = match (open, close) {
                (Some(a), Some(b)) => a.min(b),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => break,
            };
            pos += next;
            if svg[pos..].starts_with("</g>") {
                stack.pop();
                pos += 4;
                continue;
            }
            let end = pos + svg[pos..].find('>').expect("open tag ends");
            let tag = &svg[pos..end];
            if let Some(rest) = tag.strip_prefix("<g transform=\"translate(") {
                let x: f64 = rest[..rest.find(' ').expect("translate has x y")]
                    .parse()
                    .unwrap_or(0.0);
                stack.push(x);
            } else if let Some(rest) = tag.strip_prefix("<g transform=\"matrix(1 0 0 -1 ") {
                let mx: f64 = rest[..rest.find(' ').expect("matrix has x y")]
                    .parse()
                    .unwrap_or(0.0);
                let inner_end = svg[end..]
                    .find("</g>")
                    .map(|i| end + i)
                    .unwrap_or(svg.len());
                let runs = &svg[end + 1..inner_end];
                let base = stack.iter().sum::<f64>() + mx;
                let mut rest = runs;
                while let Some(i) = rest.find("x=\"") {
                    let val = &rest[i + 3..];
                    let v_end = val.find('"').expect("run offset closes");
                    let rx: f64 = val[..v_end].parse().unwrap_or(0.0);
                    min_x = min_x.min(base + rx);
                    rest = &val[v_end..];
                }
                pos = inner_end + 4; // skip this group including its `</g>`
                continue;
            }
            pos = end + 1;
        }
        min_x
    }

    /// M5 acceptance: an empty document renders exactly one blank page — the
    /// converter emits `[]` (§6.2) and the article invocation still produces a
    /// page, so the preview never shows the "nothing to render" state.
    #[test]
    fn empty_document_renders_one_blank_page() {
        let outcome = compile_markdown("", &gfm_options());
        assert_eq!(outcome.pages.len(), 1, "empty doc renders one blank page");
        assert!(
            outcome.warnings.is_empty(),
            "blank page must not warn: {:?}",
            outcome.warnings
        );
    }

    #[test]
    fn compile_preview_returns_svg_pages() {
        let outcome = compile_body("= Hello\n\nWorld.");
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        assert!(outcome.pages[0].contains("<svg"), "page must be SVG");
    }

    #[test]
    fn compile_pdf_starts_with_pdf_magic() {
        let bytes = compile_pdf_body("= Hello\n\nWorld.");
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[..4], b"%PDF", "PDF magic header");
    }

    /// Deferred issue (§13): the Align toolbar control (left/center/justified)
    /// rendered identically — the `set align(…)` / `set par(justify: …)`
    /// rules sat inside empty `if` blocks, and Typst scopes a `set` rule to
    /// its enclosing block, so neither ever reached `body`. Regression: each
    /// alignment must produce a distinct render.
    #[test]
    fn text_alignment_changes_the_render() {
        // Several lines per paragraph so justification has room to stretch
        // lines to the full text width and centering visibly shifts them.
        let markdown = [
            "A paragraph with enough words to wrap across several lines so",
            "that the justified alignment has room to stretch every line out",
            "to the full text width.",
            "",
            "A second paragraph keeps the document tall enough to matter.",
        ]
        .join("\n");

        let render = |align: TextAlign| {
            let options = RenderOptions {
                text_align: align,
                ..gfm_options()
            };
            compile_markdown(&markdown, &options).pages[0].clone()
        };

        let left = render(TextAlign::Left);
        let center = render(TextAlign::Center);
        let justified = render(TextAlign::Justified);

        assert_ne!(left, center, "centered alignment must shift line positions");
        assert_ne!(
            left, justified,
            "justified alignment must stretch lines to the full width"
        );
        assert_ne!(
            center, justified,
            "center and justified must remain distinct alignments"
        );
    }

    /// Deferred issue (§13): in center mode a bullet list rendered with the
    /// bullets pinned to the left margin while each text line centered
    /// independently — the bullet sat detached from its text. The list must
    /// instead behave as one block: items left-aligned (bullets adjacent to
    /// their text) and the whole block centered, so for a short list the
    /// bullets move well inside the page.
    #[test]
    fn center_alignment_centers_list_blocks() {
        let markdown = "- alpha\n- beta";

        let render = |align: TextAlign| {
            let options = RenderOptions {
                text_align: align,
                ..gfm_options()
            };
            compile_markdown(markdown, &options).pages[0].clone()
        };

        let left = min_glyph_x(&render(TextAlign::Left));
        let center = min_glyph_x(&render(TextAlign::Center));

        // Left: bullets at the 20mm left margin (~56.7pt on A4). Center: the
        // shrink-wrapped block is centered, so its bullets sit far inside.
        assert!(
            center > left + 50.0,
            "centered list bullets must shift well inside the page: left {left} vs center {center}"
        );
    }

    /// The task-list variant of the center-mode fix: the checkbox column must
    /// travel with its text instead of staying pinned to the left margin.
    #[test]
    fn center_alignment_centers_task_lists() {
        let markdown = "- [ ] todo\n- [x] done";

        let render = |align: TextAlign| {
            let options = RenderOptions {
                text_align: align,
                ..gfm_options()
            };
            compile_markdown(markdown, &options).pages[0].clone()
        };

        let left = min_glyph_x(&render(TextAlign::Left));
        let center = min_glyph_x(&render(TextAlign::Center));

        assert!(
            center > left + 50.0,
            "centered task-list checkboxes must shift well inside the page: left {left} vs center {center}"
        );
    }

    /// M4 acceptance: exported PDF bytes land on disk via the atomic writer.
    /// Pins the compile → write pipeline the `export_pdf` command runs behind
    /// the (headless-untestable) save dialog.
    #[test]
    fn pdf_bytes_write_atomically_to_disk() {
        let bytes = compile_pdf_body("= Hello\n\nExport me.");
        let target =
            std::env::temp_dir().join(format!("tauri-md-editor-pdf-{}.pdf", std::process::id()));

        crate::files::write_atomically(&target, &bytes).expect("must write");

        let on_disk = std::fs::read(&target).expect("file must exist on disk");
        assert_eq!(&on_disk[..4], b"%PDF", "written file must be a PDF");
        std::fs::remove_file(&target).ok();
    }

    /// §10 / M3 acceptance: a GFM table (header + body) must compile through
    /// the full pipeline (md_to_typst → static source + inputs → compile).
    /// Regression for header cells emitted as `*[a]`, which is invalid inside
    /// `#table(...)` where arguments are parsed in code mode.
    #[test]
    fn gfm_table_compiles_through_full_pipeline() {
        let markdown = "| a | b |\n|---|---|\n| 1 | 2 |";
        let outcome = compile_markdown(markdown, &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        let svg = &outcome.pages[0];
        // Text is embedded as vector glyph paths (`<use xlink:href="#…">`
        // referencing `<symbol>`s), so assert on the glyph-run count: one run
        // per table cell (a, b, 1, 2 → 4).
        let glyph_runs = svg.matches("<use xlink:href=").count();
        assert!(
            glyph_runs >= 4,
            "table cells must render text runs: {glyph_runs}"
        );
    }

    #[test]
    fn compile_error_yields_diagnostics_not_panic() {
        // The body is passed via sys.inputs and contains a deliberate error.
        let (engine, inputs) = engine_for_body("#undefined-function(42)");
        let err = compile_preview_with_inputs(&engine, &inputs).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("line") && msg.contains("undefined-function"),
            "diagnostics should carry a line number and message: {msg}"
        );
    }

    #[test]
    fn warnings_are_collected_not_fatal() {
        // An unknown font family compiles (falling back to the default font)
        // but surfaces a warning (§6.4 / §9).
        let outcome = compile_body("#set text(font: \"Nope Sans\")\nHello");
        assert!(!outcome.warnings.is_empty(), "expected a font warning");
        assert!(
            outcome.warnings.iter().any(|w| w.contains("font")),
            "warning should mention the font: {:?}",
            outcome.warnings
        );
    }

    /// M6: the engine keeps its comemo memoization cache across renders
    /// (raised eviction age for per-render speed). The cache keys on
    /// `sys.inputs`, so a reused engine must render distinct bodies
    /// correctly — never a stale preview.
    #[test]
    fn reused_engine_renders_distinct_bodies() {
        let (engine, _) = engine_for_body("unused");
        let render = |body: &str| {
            let mut inputs = Dict::new();
            inputs.insert("title".into(), Value::None);
            inputs.insert("author".into(), Value::None);
            inputs.insert("body".into(), body.into_value());
            compile_preview_with_inputs(&engine, &inputs)
                .expect("must compile")
                .pages[0]
                .clone()
        };
        let a1 = render("= Heading one");
        let a2 = render("= Heading one");
        let b = render("= Heading two");
        assert_eq!(a1, a2, "same body must be stable across renders");
        assert_ne!(a1, b, "a different body must not serve a stale cache");
    }

    /// M6: when `FontOptions.families` is set, only those families land in the
    /// engine's font book — a referenced family renders cleanly, an
    /// unreferenced one warns (proving the book is small, which is what keeps
    /// per-render compiles fast).
    #[test]
    fn subsetted_font_book_only_loads_referenced_families() {
        let mut system = typst_kit::fonts::system().map(|(_p, i)| i.family);
        let family = system.find(|f| f != "DejaVu Sans");
        let Some(family) = family else {
            return; // no usable system font on this machine
        };
        let other = system.find(|f| f != "DejaVu Sans" && f != &family);
        let Some(other) = other else {
            return; // need a second family to prove the subset is exclusive
        };
        let fonts = FontOptions {
            system_fonts: false,
            extra_dirs: Vec::new(),
            families: vec![family.clone()],
        };

        let (engine, inputs) =
            engine_for_body_with_fonts(&format!("#set text(font: {family:?})\nHello"), &fonts);
        let outcome = compile_preview_with_inputs(&engine, &inputs).expect("must compile");
        assert!(
            outcome.warnings.is_empty(),
            "the referenced family must render cleanly: {family:?} -> {:?}",
            outcome.warnings
        );

        let (engine, inputs) =
            engine_for_body_with_fonts(&format!("#set text(font: {other:?})\nHello"), &fonts);
        let outcome = compile_preview_with_inputs(&engine, &inputs).expect("must compile");
        assert!(
            !outcome.warnings.is_empty(),
            "an unreferenced family must not be in the subset: {other:?}"
        );
    }

    /// M6 acceptance: with `FontOptions { system_fonts: true }`, a family that
    /// is installed on the system but not embedded renders without the
    /// unknown-font warning; with system fonts off, the same family warns.
    /// Skips when the machine has no fonts besides our embedded DejaVu.
    #[test]
    fn system_fonts_are_used_when_enabled() {
        let family = typst_kit::fonts::system()
            .map(|(_path, info)| info.family)
            .find(|f| f != "DejaVu Sans");
        let Some(family) = family else {
            return; // no usable system font on this machine
        };
        let body = format!("#set text(font: {family:?})\nHello");

        let (engine_on, inputs) = engine_for_body_with_fonts(
            &body,
            &FontOptions {
                system_fonts: true,
                extra_dirs: Vec::new(),
                families: Vec::new(),
            },
        );
        let outcome =
            compile_preview_with_inputs(&engine_on, &inputs).expect("system font must compile");
        assert!(
            outcome.warnings.is_empty(),
            "a real system font must render without warnings: {family:?} -> {:?}",
            outcome.warnings
        );

        let (engine_off, inputs) = engine_for_body_with_fonts(&body, &FontOptions::default());
        let outcome =
            compile_preview_with_inputs(&engine_off, &inputs).expect("must still compile");
        assert!(
            !outcome.warnings.is_empty(),
            "without system fonts the family must warn: {family:?}"
        );
    }

    /// M6: a template's own top-level `#set text(font: ...)` must win over the
    /// prelude's default. The font default is a top-level prelude rule (before
    /// the user template), so the user's later rule takes effect — while a
    /// template that sets no font still renders in DejaVu Sans.
    #[test]
    fn user_top_level_font_set_overrides_prelude_default() {
        let family = typst_kit::fonts::system()
            .map(|(_path, info)| info.family)
            .find(|f| f != "DejaVu Sans");
        let Some(family) = family else {
            return; // no usable system font on this machine
        };
        let fonts = FontOptions {
            system_fonts: true,
            extra_dirs: Vec::new(),
            families: Vec::new(),
        };
        let render = |template: &str| {
            let source = build_static_source(template, &gfm_options()).unwrap();
            let (inputs, _warnings) = render_inputs("Hello world.", &gfm_options()).unwrap();
            let engine = build_engine(&source, &fonts).expect("engine");
            compile_preview_with_inputs(&engine, &inputs)
                .expect("must compile")
                .pages[0]
                .clone()
        };
        let baseline = render(DEFAULT_TEMPLATE);
        let with_font = render(&format!(
            "{DEFAULT_TEMPLATE}\n\n#set text(font: {family:?})"
        ));
        assert_ne!(
            baseline, with_font,
            "a top-level font set must take effect: {family:?}"
        );
    }

    /// Regression: a blockquote must compile through the full pipeline. The
    /// converter emits `#blockquote[...]` (§6.2), so the default template must
    /// define the helper — the startup sample document surfaced that it did
    /// not ("unknown variable: blockquote").
    #[test]
    fn blockquote_compiles_through_full_pipeline() {
        let markdown = "> Tip: add YAML front-matter — `title: My Doc`.\n>\n> Second paragraph.";
        let outcome = compile_markdown(markdown, &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
    }

    /// A task list must compile through the full pipeline *without glyph
    /// warnings*: the converter emits `sym.ballot` (☐) / `sym.ballot.check`
    /// (☑) as checkbox cells (§6.2), which requires the embedded DejaVu Sans
    /// to actually cover U+2610/U+2611.
    #[test]
    fn task_list_compiles_without_glyph_warnings() {
        let markdown = "- [ ] todo\n- [x] done";
        let outcome = compile_markdown(markdown, &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        assert!(
            outcome.warnings.is_empty(),
            "ballot glyphs must render with the embedded font: {:?}",
            outcome.warnings
        );
    }

    /// Regression: the prelude task helpers must evaluate their body argument
    /// (`[#body]`), not emit the literal text "body" (`[body]`). Two task
    /// lists with different item text (same checkbox state, so the glyphs
    /// match) must render differently — if the body were literal, both pages
    /// would render the same "body" text and the SVGs would be identical.
    #[test]
    fn task_item_content_renders_not_literal_body() {
        let render = |markdown: &str| compile_markdown(markdown, &gfm_options()).pages[0].clone();
        let aaa = render("- [ ] aaa\n- [ ] bbb");
        let body = render("- [ ] body\n- [ ] body");
        assert_ne!(
            aaa, body,
            "task item content must render, not the literal parameter name"
        );
    }

    /// The prelude pins the task-checkbox glyphs to the embedded DejaVu Sans.
    /// A font that lacks the ballot codepoints (e.g. Ubuntu) must therefore
    /// render the SAME hollow ballot glyph as the default template — not a
    /// font-fallback substitute. Besides visual consistency, this is what
    /// avoids the ~10x slower renders caused by fallback searches across the
    /// system font book.
    #[test]
    fn task_checkboxes_are_pinned_to_dejavu() {
        fn glyph_ids(svg: &str) -> Vec<String> {
            let mut ids: Vec<String> = svg
                .split("xlink:href=\"")
                .skip(1)
                .map(|s| s[..s.find('"').unwrap_or(0)].to_string())
                .collect();
            ids.sort();
            ids.dedup();
            ids
        }

        let fonts = FontOptions {
            system_fonts: true,
            extra_dirs: Vec::new(),
            families: Vec::new(),
        };
        let render = |template: &str| {
            let static_source = build_static_source(template, &gfm_options()).unwrap();
            let (inputs, _warnings) = render_inputs("- [ ]", &gfm_options()).unwrap();
            let engine = build_engine(&static_source, &fonts).expect("engine");
            compile_preview_with_inputs(&engine, &inputs)
                .expect("must compile")
                .pages[0]
                .clone()
        };
        let default_ids = glyph_ids(&render(DEFAULT_TEMPLATE));
        let ubuntu_ids = glyph_ids(&render(&format!(
            "{DEFAULT_TEMPLATE}\n\n#set text(font: \"Ubuntu\")"
        )));
        assert_eq!(
            default_ids, ubuntu_ids,
            "the ballot glyph must come from DejaVu Sans in both cases"
        );
    }

    /// The reference dark theme (`examples/dark.typ`) must compile through
    /// the full pipeline *without warnings*: it is the worked example for
    /// theme designers, so its glyph choices (`#sym.checkmark`, `#sym.circle`,
    /// the en dash) and its helper overrides must keep rendering with the
    /// embedded DejaVu Sans and the current prelude.
    #[test]
    fn dark_theme_example_compiles_without_warnings() {
        let theme_path = format!("{}/../examples/dark.typ", env!("CARGO_MANIFEST_DIR"));
        let theme = std::fs::read_to_string(&theme_path).expect("examples/dark.typ must exist");
        let template = format!("{DEFAULT_TEMPLATE}\n\n{theme}");
        let markdown = [
            "---",
            "title: Dark Probe",
            "author: Theme Designer",
            "---",
            "# White on black",
            "",
            "> A block quote on a dark card.",
            "",
            "- [x] completed task",
            "- [ ] open task",
            "- plain item mixed in",
            "",
            "Inline `code` and a code block:",
            "",
            "```rust",
            "fn main() {}",
            "```",
            "",
            "| a | b |",
            "|---|---|",
            "| 1 | 2 |",
            "",
            "1. one",
            "2. two",
            "",
            "---",
        ]
        .join("\n");
        let outcome = compile_custom_theme(&template, &markdown);
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        assert!(
            outcome.warnings.is_empty(),
            "dark theme glyphs must render with the embedded font: {:?}",
            outcome.warnings
        );
    }

    /// A footnote reference must compile through the full pipeline in the
    /// Custom flavor (§6.2): the marker is inlined at its call site and the
    /// definition renders at the page bottom.
    #[test]
    fn footnote_compiles_through_full_pipeline() {
        let markdown = "- bullet item [^1]\n- another item\n\n[^1]: the note";
        let outcome = compile_markdown(markdown, &custom_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
    }

    /// Footnote markers in the SVG preview must have `href="#..."` anchors so
    /// they are clickable — the `svg_in_bundle` path resolves `Destination::
    /// Location` items via `LateLinkResolver` (§6.4).
    ///
    /// IGNORED on typst 0.15: the paged introspector implements `anchor()`
    /// as `None` (anchors exist only in the HTML export path), so
    /// `LateLinkResolver` never resolves a Location link and the SVG
    /// renderer emits `<a>` without `href`. The resolution machinery in
    /// `compile_preview_pages` is in place and needs no change — un-ignore
    /// this once typst grows paged anchors. (The old assertion
    /// `contains("href=\"#")` was a false positive: glyph-reuse
    /// `<use href="#g…">` references matched it.)
    #[ignore = "typst 0.15 paged introspector has no anchors; SVG Location links unresolved"]
    #[test]
    fn footnote_markers_have_svg_href_anchors() {
        let markdown = "Body text.[^1]\n\n[^1]: the note";
        let outcome = compile_markdown(markdown, &custom_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        let svg = &outcome.pages[0];

        // A resolved internal link is an ANCHOR element with an href —
        // `<a href="#…">`. Glyph-reuse `<use href="#g…">` refs don't count.
        assert!(
            svg.contains("<a href=\"#"),
            "footnote marker must have an href anchor in SVG: {:?}",
            svg
        );
    }

    /// The dark theme must restyle the footnote divider: the stock separator
    /// is a black line (invisible on the dark page fill), so `dark.typ`
    /// sets `footnote.entry(separator: …)` to a visible dark gray. In a doc
    /// whose only strokes are the separator, the SVG must show the themed
    /// color and no black.
    #[test]
    fn dark_theme_footnote_separator_is_visible() {
        let theme_path = format!("{}/../examples/dark.typ", env!("CARGO_MANIFEST_DIR"));
        let theme = std::fs::read_to_string(&theme_path).expect("examples/dark.typ must exist");
        let template = format!("{DEFAULT_TEMPLATE}\n\n{theme}");
        let markdown = "Body text.[^1]\n\n[^1]: the note";
        let outcome = compile_custom_theme(&template, markdown);
        let svg = &outcome.pages[0];
        assert!(
            svg.contains("stroke=\"#30363d\""),
            "the themed separator must be drawn: {:?}",
            svg
        );
        assert!(
            !svg.contains("stroke=\"#000000\""),
            "the stock black separator must be replaced"
        );
    }

    /// Regression: the prelude `markdown-table` must lay cells out in columns.
    /// `columns: auto` is a *single* auto column, which stacks every cell
    /// vertically; the converter passes the markdown column count and the
    /// helper builds one `auto` width per column, matching the pre-prelude
    /// `#table(columns: (auto, auto), …)` layout.
    #[test]
    fn table_cells_render_in_columns() {
        let markdown = "| a | b |\n|---|---|\n| 1 | 2 |";
        let outcome = compile_markdown(markdown, &gfm_options());
        let svg = &outcome.pages[0];

        let render_ref = |body: &str| compile_body(body).pages[0].clone();
        let stacked = render_ref("#table(columns: (auto,), [*a*], [*b*], [1], [2])");
        let two_cols = render_ref("#table(columns: (auto, auto), [*a*], [*b*], [1], [2])");

        assert_ne!(svg, &stacked, "cells must not stack in a single column");
        assert_eq!(svg, &two_cols, "cells must lay out in two auto columns");
    }

    /// Mirrors the feature set of the startup sample (`lib/sample.js`) so the
    /// class of documents it represents stays compile-green: headings, text
    /// styles, links, blockquote, nested lists, task lists, code, GFM table,
    /// thematic break and soft breaks — none of which the probe tests used.
    #[test]
    fn kitchen_sink_document_compiles_through_full_pipeline() {
        let markdown = [
            "# Hello, Markdown editor",
            "",
            "**Bold**, *italic*, `inline code` and ~~strikethrough~~.",
            "",
            "[a link](https://example.com)",
            "",
            "> Tip: add YAML front-matter — `title: My Doc`.",
            "",
            "- bullet item",
            "  - nested item",
            "",
            "1. ordered item",
            "2. second item",
            "",
            "- [x] completed task",
            "- [ ] open task",
            "",
            "```rust",
            "fn main() { println!(\"hello\"); }",
            "```",
            "",
            "| Feature   | Flavor     |",
            "| --------- | ---------- |",
            "| Tables    | GFM, Custom |",
            "",
            "---",
            "",
            "*A soft break continues",
            "on the next line*",
        ]
        .join("\n");
        let outcome = compile_markdown(&markdown, &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
    }

    /// Heading IDs attach as real Typst labels (§6.2): the emitted
    /// `#label("…")` must render nothing visible (the old `{\#id}` form
    /// leaked literal text into the output) and must be a real link target
    /// (a link to a missing label is a compile error, so a clean compile
    /// proves the label exists). SVG `href` resolution itself is a typst
    /// 0.15 limitation: the paged introspector implements `anchor()` as
    /// `None`, so `LateLinkResolver` never resolves Location links.
    #[test]
    fn heading_ids_render_invisible_labels_that_links_resolve() {
        // Invisible: a heading with an ID renders the same glyphs as the
        // same heading without one.
        let mut no_ids = gfm_options();
        no_ids.heading_ids = HeadingIds { max_level: 0 };
        let plain = compile_markdown("# Title", &no_ids);
        let labeled = compile_markdown("# Title", &gfm_options());
        assert_eq!(
            plain.pages[0].matches("<path").count(),
            labeled.pages[0].matches("<path").count(),
            "the label must not render visible glyphs"
        );

        // Real link target: referencing the converted body's label compiles
        // cleanly — with the old `{\#id}` emission this failed with
        // "label `title` does not exist in the document".
        let (inputs, _warnings) = render_inputs("# Title", &gfm_options()).unwrap();
        let body = match inputs.get("body").unwrap() {
            Value::Str(s) => s.as_str().to_string(),
            other => panic!("body must be a string, got {other:?}"),
        };
        let outcome = compile_body(&format!("{body}\n\n#link(label(\"title\"))[go]"));
        assert!(!outcome.pages.is_empty(), "label link must compile");
    }

    /// `[go](#title)` targets the heading's label through the prelude's
    /// `internal-link` helper (§6.2): the link resolves in the PDF (and in
    /// the SVG once typst resolves Location links there).
    #[test]
    fn internal_link_to_heading_compiles_through_full_pipeline() {
        let outcome = compile_markdown("# Title\n\n[go](#title)", &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        // The link renders as an actual link element.
        assert!(
            outcome.pages[0].contains("<a "),
            "the internal link must render as a link element"
        );
    }

    /// A broken TOC entry (`[go](#missing)` with no such heading) must not
    /// fail the render: the prelude helper degrades it to plain text —
    /// consistent with the converter's lossy-HTML / dangling-footnote
    /// philosophy (§6.2).
    #[test]
    fn dangling_internal_link_degrades_to_plain_text() {
        let outcome = compile_markdown("# Title\n\n[go](#missing)", &gfm_options());
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        assert!(
            !outcome.pages[0].contains("<a "),
            "a dangling link must not render as a link element"
        );
    }

    #[test]
    fn custom_template_with_syntax_error_fails_probe_compile() {
        // A custom template is composed AFTER the prelude (§6.3); a broken
        // override (here: an undefined function in a helper the probe body
        // exercises) must surface as diagnostics, not a panic.
        let broken = "#let task-list(..items) = boom(1)";
        let template = format!("{DEFAULT_TEMPLATE}\n\n{broken}");
        let static_source = build_static_source(&template, &gfm_options()).unwrap();
        let (inputs, _warnings) = render_inputs("- [x] done", &gfm_options()).unwrap();
        let engine = build_engine(&static_source, &FontOptions::default()).expect("engine");
        let err = compile_preview_with_inputs(&engine, &inputs).unwrap_err();
        assert!(err.to_string().contains("line"), "{err}");
    }

    /// Code blocks render in a MONOSPACE font: a line of narrow glyphs
    /// (`iiii`) spans the same width as a line of wide glyphs (`mmmm`).
    /// Regression: with system fonts excluded from the default template's
    /// font book, `raw()`'s DejaVu Sans Mono default found no face and
    /// silently fell back to the proportional body font — code indentation
    /// and word spacing broke (commit 177c8c8 introduced the exclusion).
    #[test]
    fn code_blocks_render_in_a_monospace_font() {
        /// Rightmost glyph placement on the page — the width of the only
        /// text line (each document is a single code block).
        fn max_glyph_x(svg: &str) -> f64 {
            svg.split("<use")
                .skip(1)
                .filter_map(|frag| {
                    let start = frag.find("x=\"")? + 3;
                    let rest = &frag[start..];
                    rest[..rest.find('"')?].parse::<f64>().ok()
                })
                .fold(0.0f64, f64::max)
        }

        let narrow = compile_markdown("```\niiii\n```", &gfm_options());
        let wide = compile_markdown("```\nmmmm\n```", &gfm_options());
        let (narrow_x, wide_x) = (max_glyph_x(&narrow.pages[0]), max_glyph_x(&wide.pages[0]));
        assert!(
            (narrow_x - wide_x).abs() < 0.01,
            "monospace advances must match: iiii ends at {narrow_x}, mmmm at {wide_x}"
        );
    }
}
