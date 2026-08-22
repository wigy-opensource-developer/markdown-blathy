//! Typst template composition (§6.3 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports. The template source (embedded by default,
//! or read from a user path by the command layer) is concatenated with an
//! `#article(...)` invocation that reads the per-render data (`title`,
//! `author`, `body`) from `sys.inputs` (§7.5, M6). This split is what lets the
//! M6 engine cache reuse one compiled engine across keystrokes: the static
//! source only changes when the template or the render options change, while
//! the markdown body is passed per render via `render_inputs`.

use crate::markdown::convert::{md_to_typst, DocMeta};
use crate::options::RenderOptions;
use crate::typst_engine::engine::FontOptions;
use typst::foundations::{Dict, IntoValue};

/// The embedded default template (`templates/default.typ`).
///
/// This is the PRELUDE (§6.3): default implementations of every helper the
/// converter emits (`task-list`/`task-done`/`task-todo`/`task-plain`,
/// `blockquote`, `thematic-break`, `inline-code`, `code-block`,
/// `markdown-table`, `ordered-list`, `title-block`) plus a default `article`.
/// User templates are composed *after* it, so Typst shadowing lets them
/// override any of these while falling back to the defaults for the rest.
pub const DEFAULT_TEMPLATE: &str = include_str!("templates/default.typ");

/// Arguments for the template's `article` invocation (§6.3).
///
/// Only the *static* render options live here (they are baked into the
/// engine's main file). The per-render data — `title`, `author`, `body` —
/// travels via `sys.inputs` (`render_inputs`), so typing in the editor never
/// rebuilds the engine (M6, §7.5).
pub struct TemplateArgs<'a> {
    /// Typst `page` paper name, e.g. `"a4"` (§6.1).
    pub paper: &'a str,
    /// Horizontal page margin in mm (the smaller of left/right, see §7.4).
    pub margin_x_mm: f32,
    /// Vertical page margin in mm (the smaller of top/bottom, see §7.4).
    pub margin_y_mm: f32,
    /// `"left"` | `"center"` | `"justified"`.
    pub text_align: &'a str,
}

/// The converter-emitted prelude helpers, listed in the order the converter
/// may call them (§6.2). Every entry must be visible to the eval'd body: the
/// article invocation passes each as an explicit `scope:` binding (§6.3), so
/// a missed entry fails template validation (the probe body exercises all of
/// them). Single source of truth — the tests build the expected string from
/// this constant instead of mirroring it by hand.
pub const PRELUDE_HELPERS: &[&str] = &[
    "task-list",
    "task-done",
    "task-todo",
    "task-plain",
    "blockquote",
    "thematic-break",
    "inline-code",
    "mark",
    "superscript",
    "subscript",
    "code-block",
    "markdown-table",
    "definition-list",
    "ordered-list",
    "figure-image",
    "internal-link",
];

/// The `scope: (name: name, …)` list for the eval call: each helper resolves
/// to itself at the eval call site, so a user template's overrides win.
fn helper_scope() -> String {
    PRELUDE_HELPERS
        .iter()
        .map(|name| format!("{name}: {name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Concatenate the template source with the `#article(...)` invocation.
///
/// The invocation reads `title`/`author`/`body` from `sys.inputs` and eval's
/// the (already converted + escaped) body as markup. `eval` runs in a fresh
/// scope that only sees the standard library, so every prelude helper the
/// converter emits is passed explicitly via `scope:` — the values resolve at
/// the eval call site, so a user template's overrides win. `paper` and
/// `text_align` are emitted with Rust's `{:?}` formatting, which yields a
/// Rust string literal that is also a valid Typst string literal for these
/// ASCII values.
pub fn compose(template_source: &str, args: &TemplateArgs) -> String {
    let scope = helper_scope();
    format!(
        "{template_source}\n\n#article(\n  paper: {:?},\n  margin-x: {}mm,\n  margin-y: {}mm,\n  text-align: {:?},\n  title: sys.inputs.title,\n  author: sys.inputs.author,\n)[\n#eval(sys.inputs.body, mode: \"markup\", scope: ({scope}))\n]",
        args.paper, args.margin_x_mm, args.margin_y_mm, args.text_align,
    )
}

/// The static part of the composed source for the given template + options
/// (§7.5, M6). The engine is (re)built only when this string changes — i.e.
/// when the template is swapped or a toolbar option (paper/margins/align)
/// changes — so system fonts are scanned once per template change, not on
/// every keystroke render.
pub fn build_static_source(
    template_source: &str,
    options: &RenderOptions,
) -> Result<String, String> {
    Ok(compose(
        template_source,
        &TemplateArgs {
            paper: options.paper.typst_name(),
            // The smaller of the two horizontal/vertical margins is used for
            // the template's `x`/`y` shorthand (SPEC §7.4).
            margin_x_mm: options.margins.x_mm(),
            margin_y_mm: options.margins.y_mm(),
            text_align: options.text_align.typst_name(),
        },
    ))
}

/// The per-render `sys.inputs` dictionary, the effective document settings,
/// and converter warnings. `inputs` carries `title`/`author` (as `none` or a
/// string) and `body` (the converted + escaped Typst markup) for the
/// template; the `DocMeta` is surfaced to the frontend (§6.2). Built on every
/// render; the engine is untouched by it (M6). The command layer merges the
/// warnings with the engine's (§3).
pub fn render_inputs(
    markdown: &str,
    options: &RenderOptions,
) -> Result<(Dict, DocMeta, Vec<String>), String> {
    let converted = md_to_typst(markdown, options.flavor, &options.heading_ids);
    let meta = DocMeta {
        title: converted.title.clone(),
        author: converted.author.clone(),
        flavor: converted.flavor,
        features: converted.features,
        auto_heading_level: converted.auto_heading_level,
    };
    let warnings = converted.warnings;
    let mut inputs = Dict::new();
    inputs.insert("title".into(), converted.title.into_value());
    inputs.insert("author".into(), converted.author.into_value());
    inputs.insert("body".into(), converted.body.into_value());
    Ok((inputs, meta, warnings))
}

/// The font families a template references via `#set text(font: ...)` (§6.3).
///
/// A plain scan for `font:` followed by quoted strings — catches `font: "X"`
/// and `font: ("X", "Y")` (a `#let`-indirect font is not detected, and those
/// templates keep the full system-font scan as a fallback). The command layer
/// uses the result to load only these families into the engine's font book,
/// which keeps per-render compiles fast (a 700-face book makes them ~5×
/// slower).
pub fn referenced_font_families(source: &str) -> Vec<String> {
    let mut families: Vec<String> = Vec::new();
    let mut rest = source;
    while let Some(idx) = rest.find("font:") {
        rest = &rest[idx + 5..];
        let end = rest.find(['\n', ')']).unwrap_or(rest.len());
        for part in rest[..end].split('"').skip(1).step_by(2) {
            let name = part.trim();
            if !name.is_empty() && !families.iter().any(|f| f.eq_ignore_ascii_case(name)) {
                families.push(name.to_string());
            }
        }
    }
    families
}

/// Resolve the template to compile (§7.4): read the user template at
/// `options.template_path` — or fall back to the embedded `DEFAULT_TEMPLATE` —
/// and derive the font options for it from the same single read.
///
/// A user template is composed AFTER the prelude (§6.3), so its definitions
/// shadow the default helpers while everything it leaves out keeps the
/// prelude implementation. The error message names the offending path so the
/// UI can surface it (§9).
pub fn resolve_template(options: &RenderOptions) -> Result<(String, FontOptions), String> {
    match &options.template_path {
        Some(path) => {
            let user = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read template at {path}: {e}"))?;
            let source = format!("{DEFAULT_TEMPLATE}\n\n{user}");
            let fonts = crate::typst_engine::engine::font_options_for_source(path, &user);
            Ok((source, fonts))
        }
        None => Ok((DEFAULT_TEMPLATE.to_string(), FontOptions::default())),
    }
}

/// Everything a render needs that is not the engine itself (M6, §7.5):
/// the static source (selects the cached engine — rebuilt only when the
/// template/options change), the per-render `sys.inputs` (the markdown
/// body travels here, so typing never rebuilds the engine), the font
/// options used when the engine is (re)built, the converter warnings
/// (merged with the engine's by the command layer, §3) and the effective
/// document settings (surfaced to the frontend, §6.2).
pub struct PreparedRender {
    pub static_source: String,
    pub inputs: Dict,
    pub fonts: FontOptions,
    pub warnings: Vec<String>,
    pub doc: DocMeta,
}

/// Prepare a render: resolve the template (one file read), bake the toolbar
/// options into the static source and convert the markdown into `sys.inputs`.
///
/// This is the single setup path for both preview and export — the command
/// layer calls it instead of repeating the resolve/build/convert/fonts dance.
pub fn prepare_render(markdown: &str, options: &RenderOptions) -> Result<PreparedRender, String> {
    let (template_source, fonts) = resolve_template(options)?;
    let (inputs, doc, warnings) = render_inputs(markdown, options)?;
    Ok(PreparedRender {
        static_source: build_static_source(&template_source, options)?,
        inputs,
        fonts,
        warnings,
        doc,
    })
}

/// The probe document's `sys.inputs` (M6): a title/author pair plus a body
/// that exercises every helper the converter emits (except `figure-image`,
/// whose image file cannot exist during validation). Shared by
/// `build_and_probe` and the command layer's template validation (§7.4).
pub fn probe_inputs() -> Dict {
    let mut inputs = Dict::new();
    inputs.insert("title".into(), "Probe".into_value());
    inputs.insert("author".into(), "Probe Author".into_value());
    inputs.insert(
        "body".into(),
        [
            "= Probe heading #label(\"probe-heading\")",
            "#blockquote[Probe quote]",
            "#task-list(task-done([Done]), task-todo([Todo]), task-plain([Plain]))",
            "#thematic-break()",
            "#inline-code(\"code\")",
            "#code-block(\"rust\", \"fn main() {}\")",
            "#markdown-table(columns: 2, header: ([a], [b]), rows: (([1], [2]),))",
            "#ordered-list(1, [One], [Two])",
            "#internal-link(\"probe-heading\")[Probe link]",
            "#mark[Probe mark] #superscript[Probe sup] #subscript[Probe sub]",
            "#definition-list(([Probe term], [Probe details]))",
        ]
        .join("\n")
        .into_value(),
    );
    inputs
}

/// Build the engine for a static source and verify it compiles the probe
/// document (every converter helper plus the `title`/`author` path, §6.3/§7.4).
///
/// Returns the ready engine so the command layer can hand it to the cache —
/// validation pays the font scan exactly once and the next render reuses it.
/// Because the prelude provides defaults for all helpers, a template that only
/// restyles — or defines nothing at all — passes; the probe rejects templates
/// that do not compile at all (syntax errors) or whose *overrides* are broken
/// (e.g. a `task-list` that calls an undefined function).
///
/// Pure (no `tauri` imports): the command layer reads the file, composes the
/// static source and hands it over, so this is unit-testable with `cargo test`.
pub fn build_and_probe(
    static_source: &str,
    fonts: &FontOptions,
) -> Result<typst_as_lib::TypstEngine<typst_as_lib::TypstTemplateMainFile>, String> {
    use crate::typst_engine::engine::{build_engine, compile_preview_with_inputs};

    let engine = build_engine(static_source, fonts).map_err(|e| e.to_string())?;
    compile_preview_with_inputs(&engine, &probe_inputs()).map_err(|e| e.to_string())?;
    Ok(engine)
}

#[cfg(test)]
mod tests {
    use super::{
        build_and_probe, build_static_source, compose, prepare_render, referenced_font_families,
        render_inputs, resolve_template, TemplateArgs, DEFAULT_TEMPLATE,
    };
    use crate::options::{Flavor, HeadingIds, Margins, Paper, RenderOptions, TextAlign};
    use crate::typst_engine::engine::FontOptions;
    use typst::foundations::{Dict, IntoValue, Value};

    const TMPL: &str = "#let article(paper: \"a4\", margin-x: 20mm, margin-y: 25mm, text-align: \"left\", title: none, author: none, body) = { body }";

    /// Default render options for tests: CommonMark on A4 with the standard
    /// 20/20/20/20 mm margins, left alignment and no custom template. Tests
    /// override the fields they care about via struct-update syntax.
    fn default_options() -> RenderOptions {
        RenderOptions {
            flavor: Flavor::Commonmark,
            paper: Paper::A4,
            margins: Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            text_align: TextAlign::Left,
            template_path: None,
            heading_ids: HeadingIds::default(),
        }
    }

    #[test]
    fn compose_emits_template_and_article_invocation() {
        let args = TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
        };
        // The static source carries the toolbar options; the per-render data
        // (title/author/body) is read from sys.inputs (M6, §7.5). The scope
        // list comes from the same PRELUDE_HELPERS constant `compose` uses,
        // so adding a helper cannot drift the two apart.
        let scope = super::PRELUDE_HELPERS
            .iter()
            .map(|name| format!("{name}: {name}"))
            .collect::<Vec<_>>()
            .join(", ");
        let expected = format!(
            "{TMPL}\n\n#article(\n  paper: \"a4\",\n  margin-x: 20mm,\n  margin-y: 25mm,\n  text-align: \"left\",\n  title: sys.inputs.title,\n  author: sys.inputs.author,\n)[\n#eval(sys.inputs.body, mode: \"markup\", scope: ({scope}))\n]"
        );
        assert_eq!(compose(TMPL, &args), expected);
    }

    #[test]
    fn compose_emits_static_args_and_sys_inputs_reads() {
        let args = TemplateArgs {
            paper: "us-letter",
            margin_x_mm: 10.0,
            margin_y_mm: 15.0,
            text_align: "justified",
        };
        let out = compose(TMPL, &args);
        assert!(out.contains("paper: \"us-letter\""), "{out}");
        assert!(out.contains("margin-x: 10mm"), "{out}");
        assert!(out.contains("margin-y: 15mm"), "{out}");
        assert!(out.contains("text-align: \"justified\""), "{out}");
        assert!(out.contains("title: sys.inputs.title"), "{out}");
        assert!(out.contains("author: sys.inputs.author"), "{out}");
        assert!(
            out.contains("#eval(sys.inputs.body, mode: \"markup\", scope: ("),
            "{out}"
        );
    }

    #[test]
    fn build_static_source_maps_options() {
        let options = RenderOptions {
            flavor: Flavor::Custom,
            margins: Margins {
                top_mm: 20.0,
                bottom_mm: 25.0,
                left_mm: 15.0,
                right_mm: 15.0,
            },
            text_align: TextAlign::Justified,
            ..default_options()
        };
        let out = build_static_source(TMPL, &options).unwrap();
        assert!(out.contains("paper: \"a4\""), "{out}");
        assert!(out.contains("margin-x: 15mm"), "{out}"); // smaller of left/right
        assert!(out.contains("margin-y: 20mm"), "{out}"); // smaller of top/bottom
        assert!(out.contains("text-align: \"justified\""), "{out}");
        assert!(
            out.contains("sys.inputs"),
            "body/title/author must arrive via sys.inputs, not the static source"
        );
    }

    #[test]
    fn render_inputs_forwards_front_matter_and_body() {
        let options = RenderOptions {
            flavor: Flavor::Custom,
            ..default_options()
        };
        let (inputs, meta, warnings) =
            render_inputs("---\ntitle: \"My Doc\"\nauthor: Ada\n---\n# Hi", &options).unwrap();
        assert!(warnings.is_empty(), "no warnings for this document");
        assert_eq!(meta.title.as_deref(), Some("My Doc"));
        assert_eq!(meta.author.as_deref(), Some("Ada"));
        match inputs.get("title").unwrap() {
            Value::Str(s) => assert_eq!(s.as_str(), "My Doc"),
            other => panic!("title must be a string, got {other:?}"),
        }
        match inputs.get("author").unwrap() {
            Value::Str(s) => assert_eq!(s.as_str(), "Ada"),
            other => panic!("author must be a string, got {other:?}"),
        }
        match inputs.get("body").unwrap() {
            Value::Str(s) => assert!(
                s.as_str().contains("= Hi"),
                "body must be the converted markup"
            ),
            other => panic!("body must be a string, got {other:?}"),
        }
    }

    /// Converter warnings (e.g. duplicate heading IDs) travel alongside the
    /// inputs so the command layer can merge them with engine warnings (§3) —
    /// they must not be dropped on the floor.
    #[test]
    fn render_inputs_returns_converter_warnings() {
        let options = RenderOptions {
            flavor: Flavor::Github,
            ..default_options()
        };
        let (inputs, _meta, warnings) = render_inputs("# Section\n\n## Section", &options).unwrap();
        assert!(inputs.get("body").is_ok());
        assert!(
            warnings.iter().any(|w| w.contains("duplicate")),
            "expected the duplicate heading warning: {warnings:?}"
        );
    }

    #[test]
    fn render_inputs_emits_none_without_front_matter() {
        let options = default_options();
        let (inputs, _meta, _warnings) = render_inputs("plain text", &options).unwrap();
        assert!(matches!(inputs.get("title").ok(), Some(Value::None)));
        assert!(matches!(inputs.get("author").ok(), Some(Value::None)));
        assert!(inputs.get("body").is_ok());
    }

    /// The font-subsetting parser (§6.3): extracts the families a template
    /// references via `#set text(font: ...)`, deduplicated case-insensitively.
    #[test]
    fn referenced_font_families_extracts_font_rules() {
        assert_eq!(
            referenced_font_families("#set text(font: \"Ubuntu\")"),
            vec!["Ubuntu".to_string()]
        );
        assert_eq!(
            referenced_font_families("#set text(font: (\"Libertinus Serif\", \"C059\"))"),
            vec!["Libertinus Serif".to_string(), "C059".to_string()]
        );
        assert_eq!(
            referenced_font_families("no font: rules here"),
            Vec::<String>::new()
        );
        // Case-insensitive dedupe across multiple rules.
        assert_eq!(
            referenced_font_families("#set text(font: \"Ubuntu\")\n#set heading(font: \"ubuntu\")"),
            vec!["Ubuntu".to_string()]
        );
    }

    /// `resolve_template` (§7.4): the embedded default template is used when
    /// no `template_path` is set; a user path is read verbatim. The tuple
    /// carries the composed source plus the font options derived from the
    /// same single read.
    #[test]
    fn resolve_template_returns_default_without_path() {
        let options = default_options();
        let (source, fonts) = resolve_template(&options).expect("default must resolve");
        assert!(
            source.contains("#let article"),
            "default template must define `article`"
        );
        assert_eq!(source, DEFAULT_TEMPLATE);
        assert_eq!(fonts.families, Vec::<String>::new(), "no families to load");
        assert!(!fonts.system_fonts, "default template needs no system scan");
    }

    #[test]
    fn resolve_template_reads_user_template() {
        let path = std::env::temp_dir().join(format!(
            "tauri-md-editor-template-test-{}.typ",
            std::process::id()
        ));
        let user = "#set text(font: \"Ubuntu\")";
        std::fs::write(&path, user).unwrap();
        let options = RenderOptions {
            template_path: Some(path.to_string_lossy().into_owned()),
            ..default_options()
        };
        let (source, fonts) = resolve_template(&options).expect("user template must resolve");
        // The user template is composed AFTER the prelude so its definitions
        // shadow the defaults (§6.3).
        assert!(
            source.starts_with(DEFAULT_TEMPLATE),
            "prelude must be prepended"
        );
        assert!(
            source.trim_end().ends_with(user),
            "user source must come after the prelude"
        );
        // The font family scan runs on the same read.
        assert_eq!(fonts.families, vec!["Ubuntu".to_string()]);
        assert!(!fonts.system_fonts, "referenced families subset the book");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolve_template_reports_missing_path() {
        let options = RenderOptions {
            template_path: Some("/nonexistent/tauri-md-editor/does-not-exist.typ".to_string()),
            ..default_options()
        };
        let err = resolve_template(&options).unwrap_err();
        assert!(
            err.contains("does-not-exist.typ"),
            "error should mention the offending path: {err}"
        );
    }

    /// `prepare_render` (§7.5, M6): the shared setup path for preview and
    /// export — one template read producing the static source, the per-render
    /// inputs and the font options.
    #[test]
    fn prepare_render_bundles_static_source_inputs_and_fonts() {
        let options = RenderOptions {
            flavor: Flavor::Github,
            ..default_options()
        };
        let prepared = prepare_render("# Hi", &options).expect("must prepare");
        // Static source carries the toolbar options and the sys.inputs reads…
        assert!(
            prepared.static_source.contains("paper: \"a4\""),
            "{}",
            prepared.static_source
        );
        assert!(
            prepared.static_source.contains("sys.inputs"),
            "{}",
            prepared.static_source
        );
        // …the converted body travels in the inputs, never the static source.
        match prepared.inputs.get("body").unwrap() {
            Value::Str(s) => assert_eq!(s.as_str(), "= Hi #label(\"hi\")"),
            other => panic!("body must be a string, got {other:?}"),
        }
        assert_eq!(prepared.fonts.families, Vec::<String>::new());
    }

    /// §10: the default template must define `article` with the required
    /// parameters — verified by actually compiling a probe document.
    #[test]
    fn default_template_compiles_a_probe_document() {
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
        inputs.insert("title".into(), "Probe".into_value());
        inputs.insert("author".into(), Value::None);
        inputs.insert("body".into(), "= Heading\n\nSome *text*.".into_value());
        let engine =
            crate::typst_engine::engine::build_engine(&source, &FontOptions::default()).unwrap();
        let outcome = crate::typst_engine::engine::compile_preview_with_inputs(&engine, &inputs)
            .expect("default template probe must compile");
        assert!(
            !outcome.pages.is_empty(),
            "probe must render at least one page"
        );
        assert!(outcome.pages[0].contains("<svg"), "page must be SVG");
    }

    /// Probe a user template exactly as the `validate_template` command does
    /// (§7.4): compose it after the prelude and build a static source from
    /// real render options, then build + probe-compile the engine.
    fn probe_user_template(user_source: &str) -> Result<(), String> {
        let template = format!("{DEFAULT_TEMPLATE}\n\n{user_source}");
        let static_source =
            build_static_source(&template, &default_options()).expect("static source");
        build_and_probe(&static_source, &FontOptions::default()).map(|_| ())
    }

    /// A user template that only restyles — no `article`, no helper overrides
    /// — is accepted: the prelude provides every default (§6.3, M5+prelude).
    #[test]
    fn probe_accepts_styling_only_template() {
        probe_user_template("// styling tweaks only").expect("must pass via prelude defaults");
    }

    /// A user template may override individual helpers; a valid override
    /// passes the probe and wins over the prelude default (shadowing).
    #[test]
    fn probe_accepts_helper_override() {
        probe_user_template("#let task-todo(body) = ([#text(fill: red)[TODO]], [body])")
            .expect("valid override must pass");
    }

    /// A template with a syntax error is rejected — the prelude cannot make
    /// broken user code compile.
    #[test]
    fn probe_rejects_syntax_error() {
        let err = probe_user_template("#let x = =").unwrap_err();
        assert!(
            err.contains("line"),
            "probe failure should carry a diagnostic: {err}"
        );
    }

    /// A template whose *override* of a helper is broken (references an
    /// undefined function) is rejected because the probe body exercises every
    /// helper — the override is actually used, so its error surfaces.
    #[test]
    fn probe_rejects_broken_override() {
        let err = probe_user_template("#let task-list(..items) = boom(1)").unwrap_err();
        assert!(
            err.contains("boom"),
            "broken override must surface its own error: {err}"
        );
    }

    /// A template whose `article` override is broken is rejected — the probe
    /// exercises the title/author path, so a broken override surfaces instead
    /// of silently keeping the default.
    #[test]
    fn probe_rejects_broken_article_override() {
        let err =
            probe_user_template("#let article(..) = undefined-title-thing(title)").unwrap_err();
        assert!(
            err.contains("line"),
            "probe failure should carry a diagnostic: {err}"
        );
    }
}
