//! Typst compile & export (§6.4 of SPEC.md).
//!
//! Uses `typst-as-lib` 0.16 so we never hand-write a `typst::World`. Pure Rust
//! — no `tauri` imports. Two public entry points, both built on
//! `compile_document`:
//!
//! - `compile_preview`: per-page SVG strings for the live preview pane.
//! - `compile_pdf`: PDF bytes for export.

use std::error::Error;
use std::fmt;

use typst_as_lib::{TypstEngine, TypstTemplateMainFile};
use typst_layout::PagedDocument;

/// Embedded default fonts (DejaVu Sans, permissive license — §6.3).
pub const FONT_BYTES: &[u8] = include_bytes!("../../fonts/DejaVuSans.ttf");
pub const FONT_BOLD_BYTES: &[u8] = include_bytes!("../../fonts/DejaVuSans-Bold.ttf");

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

/// Compile a complete Typst source into a paged document.
///
/// In Typst 0.15 the document type is `PagedDocument` (typst-layout);
/// `typst::Document` is now a trait implemented by export targets.
fn compile_document(source: &str) -> Result<(PagedDocument, Vec<String>), EngineError> {
    let engine = TypstEngine::builder()
        .main_file(source) // source CONTENT, not a path
        .fonts([FONT_BYTES, FONT_BOLD_BYTES])
        .build();

    let warned = engine.compile();
    let doc = warned
        .output
        .map_err(|e| EngineError::Compile(render_error(&engine, &e)))?;
    let warnings = warned
        .warnings
        .iter()
        .map(|diag| render_diagnostic(&engine, diag))
        .collect();
    Ok((doc, warnings))
}

/// Render preview pages as SVG (§6.4).
pub fn compile_preview(source: &str) -> Result<PreviewOutcome, EngineError> {
    let (doc, warnings) = compile_document(source)?;
    // typst-svg 0.15: `svg(page: &Page, opts: &SvgOptions)` takes the whole
    // page, not `&page.frame` as in older versions.
    let pages = doc
        .pages()
        .iter()
        .map(|page| typst_svg::svg(page, &Default::default()))
        .collect();
    Ok(PreviewOutcome { pages, warnings })
}

/// Export the document as PDF bytes (§6.4).
pub fn compile_pdf(source: &str) -> Result<Vec<u8>, EngineError> {
    let (doc, _warnings) = compile_document(source)?;
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
    use super::{compile_pdf, compile_preview};
    use crate::options::{Flavor, Margins, Paper, RenderOptions, TextAlign};
    use crate::typst_engine::template::{build_source, compose, TemplateArgs, DEFAULT_TEMPLATE};

    fn probe_source(body: &str) -> String {
        compose(
            DEFAULT_TEMPLATE,
            &TemplateArgs {
                paper: "a4",
                margin_x_mm: 20.0,
                margin_y_mm: 25.0,
                text_align: "left",
                title: None,
                author: None,
                body,
            },
        )
    }

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
        }
    }

    fn custom_options() -> RenderOptions {
        RenderOptions {
            flavor: Flavor::Custom,
            ..gfm_options()
        }
    }

    /// M5 acceptance: an empty document renders exactly one blank page — the
    /// converter emits `[]` (§6.2) and the article invocation still produces a
    /// page, so the preview never shows the "nothing to render" state.
    #[test]
    fn empty_document_renders_one_blank_page() {
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), "").unwrap();
        let outcome = compile_preview(&source).expect("empty document must compile");
        assert_eq!(outcome.pages.len(), 1, "empty doc renders one blank page");
        assert!(
            outcome.warnings.is_empty(),
            "blank page must not warn: {:?}",
            outcome.warnings
        );
    }

    #[test]
    fn compile_preview_returns_svg_pages() {
        let source = probe_source("= Hello\n\nWorld.");
        let outcome = compile_preview(&source).expect("must compile");
        assert!(!outcome.pages.is_empty(), "expected at least one page");
        assert!(outcome.pages[0].contains("<svg"), "page must be SVG");
    }

    #[test]
    fn compile_pdf_starts_with_pdf_magic() {
        let source = probe_source("= Hello\n\nWorld.");
        let bytes = compile_pdf(&source).expect("must compile");
        assert!(!bytes.is_empty());
        assert_eq!(&bytes[..4], b"%PDF", "PDF magic header");
    }

    /// M4 acceptance: exported PDF bytes land on disk via the atomic writer.
    /// Pins the compile → write pipeline the `export_pdf` command runs behind
    /// the (headless-untestable) save dialog.
    #[test]
    fn pdf_bytes_write_atomically_to_disk() {
        let source = probe_source("= Hello\n\nExport me.");
        let bytes = compile_pdf(&source).expect("must compile");
        let target =
            std::env::temp_dir().join(format!("tauri-md-editor-pdf-{}.pdf", std::process::id()));

        crate::files::write_atomically(&target, &bytes).expect("must write");

        let on_disk = std::fs::read(&target).expect("file must exist on disk");
        assert_eq!(&on_disk[..4], b"%PDF", "written file must be a PDF");
        std::fs::remove_file(&target).ok();
    }

    /// §10 / M3 acceptance: a GFM table (header + body) must compile through
    /// the full pipeline (md_to_typst → build_source → compile). Regression
    /// for header cells emitted as `*[a]`, which is invalid inside `#table(...)`
    /// where arguments are parsed in code mode.
    #[test]
    fn gfm_table_compiles_through_full_pipeline() {
        let markdown = "| a | b |\n|---|---|\n| 1 | 2 |";
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("table document must compile");
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
        // `article` is invoked with a body containing a deliberate error.
        let source = probe_source("#undefined-function(42)");
        let err = compile_preview(&source).unwrap_err();
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
        let source = probe_source("#set text(font: \"Nope Sans\")\nHello");
        let outcome = compile_preview(&source).expect("must still compile");
        assert!(!outcome.warnings.is_empty(), "expected a font warning");
        assert!(
            outcome.warnings.iter().any(|w| w.contains("font")),
            "warning should mention the font: {:?}",
            outcome.warnings
        );
    }

    /// Regression: a blockquote must compile through the full pipeline. The
    /// converter emits `#blockquote[...]` (§6.2), so the default template must
    /// define the helper — the startup sample document surfaced that it did
    /// not ("unknown variable: blockquote").
    #[test]
    fn blockquote_compiles_through_full_pipeline() {
        let markdown = "> Tip: add YAML front-matter — `title: My Doc`.\n>\n> Second paragraph.";
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("blockquote document must compile");
        assert!(!outcome.pages.is_empty(), "expected at least one page");
    }

    /// A task list must compile through the full pipeline *without glyph
    /// warnings*: the converter emits `sym.ballot` (☐) / `sym.ballot.check`
    /// (☑) as checkbox cells (§6.2), which requires the embedded DejaVu Sans
    /// to actually cover U+2610/U+2611.
    #[test]
    fn task_list_compiles_without_glyph_warnings() {
        let markdown = "- [ ] todo\n- [x] done";
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("task list must compile");
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
        let render = |markdown: &str| {
            let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), markdown).unwrap();
            let outcome = compile_preview(&source).expect("task list must compile");
            outcome.pages[0].clone()
        };
        let aaa = render("- [ ] aaa\n- [ ] bbb");
        let body = render("- [ ] body\n- [ ] body");
        assert_ne!(
            aaa, body,
            "task item content must render, not the literal parameter name"
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
        let source = build_source(&template, &custom_options(), &markdown).unwrap();
        let outcome = compile_preview(&source).expect("dark theme must compile");
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
        let source = build_source(DEFAULT_TEMPLATE, &custom_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("footnote document must compile");
        assert!(!outcome.pages.is_empty(), "expected at least one page");
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
        let source = build_source(&template, &custom_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("dark theme footnote must compile");
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
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), markdown).unwrap();
        let outcome = compile_preview(&source).expect("table must compile");
        let svg = &outcome.pages[0];

        let render_ref = |body: &str| {
            compile_preview(&probe_source(body))
                .expect("reference table must compile")
                .pages[0]
                .clone()
        };
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
        let source = build_source(DEFAULT_TEMPLATE, &gfm_options(), &markdown).unwrap();
        let outcome = compile_preview(&source).expect("kitchen sink must compile");
        assert!(!outcome.pages.is_empty(), "expected at least one page");
    }

    #[test]
    fn custom_template_with_syntax_error_fails_probe_compile() {
        // A custom template is composed AFTER the prelude (§6.3); a broken
        // override (here: an undefined function in a helper the probe body
        // exercises) must surface as diagnostics, not a panic.
        let broken = "#let task-list(..items) = boom(1)";
        let source = compose(
            &format!("{DEFAULT_TEMPLATE}\n\n{broken}"),
            &TemplateArgs {
                paper: "a4",
                margin_x_mm: 20.0,
                margin_y_mm: 25.0,
                text_align: "left",
                title: None,
                author: None,
                body: "#task-list(task-todo([x]))",
            },
        );
        let err = compile_preview(&source).unwrap_err();
        assert!(err.to_string().contains("line"), "{err}");
    }
}
