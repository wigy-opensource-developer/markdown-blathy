//! Typst template composition (§6.3 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports. The template source (embedded by default,
//! or read from a user path by the command layer) is concatenated with an
//! `#article(...)` invocation that reads the per-render data (`title`,
//! `author`, `body`) from `sys.inputs` (§7.5, M6). This split is what lets the
//! M6 engine cache reuse one compiled engine across keystrokes: the static
//! source only changes when the template or the render options change, while
//! the markdown body is passed per render via `render_inputs`.

use crate::markdown::convert::md_to_typst;
use crate::options::{RenderOptions, TextAlign};
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
///
/// Keep the scope list in sync with the converter's emissions (§6.2): the
/// probe body exercises every helper, so a missed entry fails validation.
pub fn compose(template_source: &str, args: &TemplateArgs) -> String {
    let scope = [
        "task-list: task-list",
        "task-done: task-done",
        "task-todo: task-todo",
        "task-plain: task-plain",
        "blockquote: blockquote",
        "thematic-break: thematic-break",
        "inline-code: inline-code",
        "code-block: code-block",
        "markdown-table: markdown-table",
        "ordered-list: ordered-list",
        "figure-image: figure-image",
    ]
    .join(", ");
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
            margin_x_mm: options.margins.left_mm.min(options.margins.right_mm),
            margin_y_mm: options.margins.top_mm.min(options.margins.bottom_mm),
            text_align: match options.text_align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Justified => "justified",
            },
        },
    ))
}

/// The per-render `sys.inputs` dictionary: `title`/`author` from the YAML
/// front matter (as `none` or a string) and `body` (the converted + escaped
/// Typst markup). Built on every render; the engine is untouched by it (M6).
pub fn render_inputs(markdown: &str, options: &RenderOptions) -> Result<Dict, String> {
    let converted = md_to_typst(markdown, options.flavor);
    let mut inputs = Dict::new();
    inputs.insert("title".into(), converted.title.into_value());
    inputs.insert("author".into(), converted.author.into_value());
    inputs.insert("body".into(), converted.body.into_value());
    Ok(inputs)
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

/// Resolve the template source to compile (§7.4): read the user template at
/// `options.template_path`, or fall back to the embedded `DEFAULT_TEMPLATE`.
///
/// A user template is composed AFTER the prelude (§6.3), so its definitions
/// shadow the default helpers while everything it leaves out keeps the
/// prelude implementation. The error message names the offending path so the
/// UI can surface it (§9).
pub fn resolve_template_source(options: &RenderOptions) -> Result<String, String> {
    match &options.template_path {
        Some(path) => {
            let user = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read template at {path}: {e}"))?;
            Ok(format!("{DEFAULT_TEMPLATE}\n\n{user}"))
        }
        None => Ok(DEFAULT_TEMPLATE.to_string()),
    }
}

/// The probe document's `sys.inputs` (M6): a title/author pair plus a body
/// that exercises every helper the converter emits (except `figure-image`,
/// whose image file cannot exist during validation). Shared by
/// `probe_template` and the command layer's template validation (§7.4).
pub fn probe_inputs() -> Dict {
    let mut inputs = Dict::new();
    inputs.insert("title".into(), "Probe".into_value());
    inputs.insert("author".into(), "Probe Author".into_value());
    inputs.insert(
        "body".into(),
        [
            "= Probe heading",
            "#blockquote[Probe quote]",
            "#task-list(task-done([Done]), task-todo([Todo]), task-plain([Plain]))",
            "#thematic-break()",
            "#inline-code(\"code\")",
            "#code-block(\"rust\", \"fn main() {}\")",
            "#markdown-table(columns: 2, header: ([a], [b]), rows: (([1], [2]),))",
            "#ordered-list(1, [One], [Two])",
        ]
        .join("\n")
        .into_value(),
    );
    inputs
}

/// Validate a user-supplied template (§6.3, §7.4). The template source is
/// composed AFTER the prelude and compiled against a probe document that
/// exercises every helper the converter emits plus the `title`/`author` path.
///
/// Because the prelude provides defaults for all helpers, a template that
/// only restyles — or defines nothing at all — passes; the probe rejects
/// templates that do not compile at all (syntax errors) or whose *overrides*
/// are broken (e.g. a `task-list` that calls an undefined function).
///
/// Pure (no `tauri` imports): the command layer reads the file and hands the
/// source over, so this is unit-testable without a running app. `fonts`
/// mirrors how the template will actually render (M6) — most importantly
/// whether system fonts / a font folder are available.
pub fn probe_template(
    user_source: &str,
    fonts: &crate::typst_engine::engine::FontOptions,
) -> Result<(), String> {
    use crate::typst_engine::engine::{build_engine, compile_preview_with_inputs};

    let composed = format!("{DEFAULT_TEMPLATE}\n\n{user_source}");
    let source = compose(
        &composed,
        &TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
        },
    );
    let engine = build_engine(&source, fonts).map_err(|e| e.to_string())?;
    compile_preview_with_inputs(&engine, &probe_inputs())
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_static_source, compose, probe_template, referenced_font_families, render_inputs,
        resolve_template_source, TemplateArgs, DEFAULT_TEMPLATE,
    };
    use crate::options::{Flavor, Margins, Paper, RenderOptions, TextAlign};
    use crate::typst_engine::engine::FontOptions;
    use typst::foundations::{Dict, IntoValue, Value};

    const TMPL: &str = "#let article(paper: \"a4\", margin-x: 20mm, margin-y: 25mm, text-align: \"left\", title: none, author: none, body) = { body }";

    fn options(
        flavor: Flavor,
        paper: Paper,
        margins: Margins,
        text_align: TextAlign,
        template_path: Option<String>,
    ) -> RenderOptions {
        RenderOptions {
            flavor,
            paper,
            margins,
            text_align,
            template_path,
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
        // (title/author/body) is read from sys.inputs (M6, §7.5).
        let scope = [
            "task-list: task-list",
            "task-done: task-done",
            "task-todo: task-todo",
            "task-plain: task-plain",
            "blockquote: blockquote",
            "thematic-break: thematic-break",
            "inline-code: inline-code",
            "code-block: code-block",
            "markdown-table: markdown-table",
            "ordered-list: ordered-list",
            "figure-image: figure-image",
        ]
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
        let options = options(
            Flavor::Custom,
            Paper::A4,
            Margins {
                top_mm: 20.0,
                bottom_mm: 25.0,
                left_mm: 15.0,
                right_mm: 15.0,
            },
            TextAlign::Justified,
            None,
        );
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
        let options = options(
            Flavor::Custom,
            Paper::A4,
            Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            TextAlign::Left,
            None,
        );
        let inputs =
            render_inputs("---\ntitle: \"My Doc\"\nauthor: Ada\n---\n# Hi", &options).unwrap();
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

    #[test]
    fn render_inputs_emits_none_without_front_matter() {
        let options = options(
            Flavor::Commonmark,
            Paper::A4,
            Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            TextAlign::Left,
            None,
        );
        let inputs = render_inputs("plain text", &options).unwrap();
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

    /// `resolve_template_source` (§7.4): the embedded default template is used
    /// when no `template_path` is set; a user path is read verbatim.
    #[test]
    fn resolve_template_source_returns_default_without_path() {
        let options = RenderOptions {
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
        };
        let source = resolve_template_source(&options).expect("default must resolve");
        assert!(
            source.contains("#let article"),
            "default template must define `article`"
        );
        assert_eq!(source, DEFAULT_TEMPLATE);
    }

    #[test]
    fn resolve_template_source_reads_user_template() {
        let path = std::env::temp_dir().join(format!(
            "tauri-md-editor-template-test-{}.typ",
            std::process::id()
        ));
        let user = "#let task-todo(body) = ([TODO], [body])";
        std::fs::write(&path, user).unwrap();
        let options = RenderOptions {
            flavor: Flavor::Commonmark,
            paper: Paper::A4,
            margins: Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            text_align: TextAlign::Left,
            template_path: Some(path.to_string_lossy().into_owned()),
        };
        let source = resolve_template_source(&options).expect("user template must resolve");
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
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolve_template_source_reports_missing_path() {
        let options = RenderOptions {
            flavor: Flavor::Commonmark,
            paper: Paper::A4,
            margins: Margins {
                top_mm: 20.0,
                bottom_mm: 20.0,
                left_mm: 20.0,
                right_mm: 20.0,
            },
            text_align: TextAlign::Left,
            template_path: Some("/nonexistent/tauri-md-editor/does-not-exist.typ".to_string()),
        };
        let err = resolve_template_source(&options).unwrap_err();
        assert!(
            err.contains("does-not-exist.typ"),
            "error should mention the offending path: {err}"
        );
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

    /// A user template that only restyles — no `article`, no helper overrides
    /// — is accepted: the prelude provides every default (§6.3, M5+prelude).
    #[test]
    fn probe_template_accepts_styling_only_template() {
        probe_template("// styling tweaks only", &FontOptions::default())
            .expect("must pass via prelude defaults");
    }

    /// A user template may override individual helpers; a valid override
    /// passes the probe and wins over the prelude default (shadowing).
    #[test]
    fn probe_template_accepts_helper_override() {
        probe_template(
            "#let task-todo(body) = ([#text(fill: red)[TODO]], [body])",
            &FontOptions::default(),
        )
        .expect("valid override must pass");
    }

    /// A template with a syntax error is rejected — the prelude cannot make
    /// broken user code compile.
    #[test]
    fn probe_template_rejects_syntax_error() {
        let err = probe_template("#let x = =", &FontOptions::default()).unwrap_err();
        assert!(
            err.contains("line"),
            "probe failure should carry a diagnostic: {err}"
        );
    }

    /// A template whose *override* of a helper is broken (references an
    /// undefined function) is rejected because the probe body exercises every
    /// helper — the override is actually used, so its error surfaces.
    #[test]
    fn probe_template_rejects_broken_override() {
        let err = probe_template("#let task-list(..items) = boom(1)", &FontOptions::default())
            .unwrap_err();
        assert!(
            err.contains("boom"),
            "broken override must surface its own error: {err}"
        );
    }

    /// A template whose `article` override is broken is rejected — the probe
    /// exercises the title/author path, so a broken override surfaces instead
    /// of silently keeping the default.
    #[test]
    fn probe_template_rejects_broken_article_override() {
        let tmpl = "#let article(..) = undefined-title-thing(title)";
        let err = probe_template(tmpl, &FontOptions::default()).unwrap_err();
        assert!(
            err.contains("line"),
            "probe failure should carry a diagnostic: {err}"
        );
    }
}
