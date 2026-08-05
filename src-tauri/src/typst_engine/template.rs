//! Typst template composition (§6.3 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports. The template source (embedded by default,
//! or read from a user path by the command layer) is concatenated with an
//! `#article(...)[body]` invocation carrying the render options and the
//! already-escaped markdown body.

use crate::markdown::convert::md_to_typst;
use crate::options::{RenderOptions, TextAlign};

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
pub struct TemplateArgs<'a> {
    /// Typst `page` paper name, e.g. `"a4"` (§6.1).
    pub paper: &'a str,
    /// Horizontal page margin in mm (the smaller of left/right, see §7.4).
    pub margin_x_mm: f32,
    /// Vertical page margin in mm (the smaller of top/bottom, see §7.4).
    pub margin_y_mm: f32,
    /// `"left"` | `"center"` | `"justified"`.
    pub text_align: &'a str,
    /// Front-matter title, if any (§6.2).
    pub title: Option<&'a str>,
    /// Front-matter author, if any (§6.2).
    pub author: Option<&'a str>,
    /// Output of `md_to_typst` (already escaped markup, §6.2).
    pub body: &'a str,
}

/// Concatenate the template source with an `#article(...)[body]` invocation.
///
/// `paper` and `text_align` are emitted with Rust's `{:?}` formatting, which
/// yields a Rust string literal that is also a valid Typst string literal for
/// these ASCII values. `title`/`author` are either `none` or a `{:?}`-quoted
/// string literal (user content must be quoted; `{:?}` escapes `"`).
pub fn compose(template_source: &str, args: &TemplateArgs) -> String {
    // `{:?}` on `&str` yields a Rust string literal that is also a valid
    // Typst string literal for these ASCII values (SPEC §6.3). For title/
    // author (user content) the quoting also escapes any embedded `"`.
    let title = args
        .title
        .map(|t| format!("{t:?}"))
        .unwrap_or_else(|| "none".to_string());
    let author = args
        .author
        .map(|a| format!("{a:?}"))
        .unwrap_or_else(|| "none".to_string());
    format!(
        "{template_source}\n\n#article(\n  paper: {:?},\n  margin-x: {}mm,\n  margin-y: {}mm,\n  text-align: {:?},\n  title: {title},\n  author: {author},\n)[\n{}\n]",
        args.paper, args.margin_x_mm, args.margin_y_mm, args.text_align, args.body,
    )
}

/// Convert markdown and compose it with the given template source (§7.4).
///
/// The smaller of the two horizontal/vertical margins is used for the
/// template's `x`/`y` shorthand. This pins that choice (SPEC §7.4 allows
/// either, the test suite must pin the behaviour).
pub fn build_source(
    template_source: &str,
    options: &RenderOptions,
    markdown: &str,
) -> Result<String, String> {
    // md_to_typst is infallible in v1 (SPEC §6.2); the Result is kept for
    // spec §7.4 parity and future failure modes.
    let converted = md_to_typst(markdown, options.flavor);
    Ok(compose(
        template_source,
        &TemplateArgs {
            paper: options.paper.typst_name(),
            margin_x_mm: options.margins.left_mm.min(options.margins.right_mm),
            margin_y_mm: options.margins.top_mm.min(options.margins.bottom_mm),
            text_align: match options.text_align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Justified => "justified",
            },
            title: converted.title.as_deref(),
            author: converted.author.as_deref(),
            body: &converted.body,
        },
    ))
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

/// Validate a user-supplied template (§6.3, §7.4). The template source is
/// composed AFTER the prelude and compiled against a probe document that
/// exercises every helper the converter emits (except `figure-image`, whose
/// image file cannot exist during validation) plus the `title`/`author` path.
///
/// Because the prelude provides defaults for all helpers, a template that
/// only restyles — or defines nothing at all — passes; the probe rejects
/// templates that do not compile at all (syntax errors) or whose *overrides*
/// are broken (e.g. a `task-list` that calls an undefined function).
///
/// Pure (no `tauri` imports): the command layer reads the file and hands the
/// source over, so this is unit-testable without a running app.
pub fn probe_template(user_source: &str) -> Result<(), String> {
    let composed = format!("{DEFAULT_TEMPLATE}\n\n{user_source}");
    let probe = compose(
        &composed,
        &TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
            title: Some("Probe"),
            author: Some("Probe Author"),
            body: &[
                "= Probe heading",
                "#blockquote[Probe quote]",
                "#task-list(task-done([Done]), task-todo([Todo]), task-plain([Plain]))",
                "#thematic-break()",
                "#inline-code(\"code\")",
                "#code-block(\"rust\", \"fn main() {}\")",
                "#markdown-table(columns: 2, header: ([a], [b]), rows: (([1], [2]),))",
                "#ordered-list(1, [One], [Two])",
            ]
            .join("\n"),
        },
    );
    crate::typst_engine::engine::compile_preview(&probe)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_source, compose, probe_template, resolve_template_source, TemplateArgs,
        DEFAULT_TEMPLATE,
    };
    use crate::options::{Flavor, Margins, Paper, RenderOptions, TextAlign};

    const TMPL: &str = "#let article(paper: \"a4\", margin-x: 20mm, margin-y: 25mm, text-align: \"left\", title: none, author: none, body) = { body }";

    #[test]
    fn compose_emits_template_and_article_invocation() {
        let args = TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
            title: None,
            author: None,
            body: "Hello",
        };
        let expected = format!(
            "{TMPL}\n\n#article(\n  paper: \"a4\",\n  margin-x: 20mm,\n  margin-y: 25mm,\n  text-align: \"left\",\n  title: none,\n  author: none,\n)[\nHello\n]"
        );
        assert_eq!(compose(TMPL, &args), expected);
    }

    #[test]
    fn compose_passes_body_through_verbatim() {
        // The body arrives already escaped (§6.2); compose must not touch it.
        let body = "a \\# b\n#figure(image(\"x.png\"))";
        let args = TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
            title: None,
            author: None,
            body,
        };
        let out = compose(TMPL, &args);
        assert!(out.contains(&format!("[\n{body}\n]")), "{out}");
        assert!(!out.contains("\\\\#"), "body must not be re-escaped: {out}");
    }

    #[test]
    fn compose_quotes_title_and_author_when_present() {
        let args = TemplateArgs {
            paper: "us-letter",
            margin_x_mm: 10.0,
            margin_y_mm: 15.0,
            text_align: "justified",
            title: Some("My \"Great\" Title"),
            author: Some("Jane Doe"),
            body: "body",
        };
        let out = compose(TMPL, &args);
        assert!(out.contains("title: \"My \\\"Great\\\" Title\""), "{out}");
        assert!(out.contains("author: \"Jane Doe\""), "{out}");
        assert!(out.contains("paper: \"us-letter\""), "{out}");
        assert!(out.contains("text-align: \"justified\""), "{out}");
        assert!(out.contains("margin-x: 10mm"), "{out}");
        assert!(out.contains("margin-y: 15mm"), "{out}");
    }

    #[test]
    fn compose_emits_none_when_title_and_author_absent() {
        let args = TemplateArgs {
            paper: "a4",
            margin_x_mm: 20.0,
            margin_y_mm: 25.0,
            text_align: "left",
            title: None,
            author: None,
            body: "body",
        };
        let out = compose(TMPL, &args);
        assert!(out.contains("title: none,"), "{out}");
        assert!(out.contains("author: none,"), "{out}");
        assert!(!out.contains("title: \"none\""), "{out}");
    }

    #[test]
    fn build_source_maps_options_and_forwards_front_matter() {
        let options = RenderOptions {
            // Front matter is only parsed in the Custom flavor (§6.1).
            flavor: Flavor::Custom,
            paper: Paper::A4,
            margins: Margins {
                top_mm: 20.0,
                bottom_mm: 25.0,
                left_mm: 15.0,
                right_mm: 15.0,
            },
            text_align: TextAlign::Justified,
            template_path: None,
        };
        let markdown = "---\ntitle: A Title\nauthor: Me\n---\n# Hi";
        let out = build_source(TMPL, &options, markdown).unwrap();
        assert!(out.contains("paper: \"a4\""), "{out}");
        assert!(out.contains("margin-x: 15mm"), "{out}");
        assert!(out.contains("margin-y: 20mm"), "{out}");
        assert!(out.contains("text-align: \"justified\""), "{out}");
        assert!(out.contains("title: \"A Title\""), "{out}");
        assert!(out.contains("author: \"Me\""), "{out}");
        assert!(out.contains("= Hi"), "{out}");
    }

    #[test]
    fn build_source_uses_none_when_no_front_matter() {
        let options = RenderOptions {
            flavor: Flavor::Commonmark,
            paper: Paper::A5,
            margins: Margins {
                top_mm: 10.0,
                bottom_mm: 10.0,
                left_mm: 10.0,
                right_mm: 10.0,
            },
            text_align: TextAlign::Left,
            template_path: None,
        };
        let out = build_source(TMPL, &options, "plain text").unwrap();
        assert!(out.contains("title: none,"), "{out}");
        assert!(out.contains("author: none,"), "{out}");
        assert!(out.contains("paper: \"a5\""), "{out}");
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
                title: Some("Probe"),
                author: None,
                body: "= Heading\n\nSome *text*.",
            },
        );
        let outcome = crate::typst_engine::engine::compile_preview(&source)
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
        probe_template("// styling tweaks only").expect("must pass via prelude defaults");
    }

    /// A user template may override individual helpers; a valid override
    /// passes the probe and wins over the prelude default (shadowing).
    #[test]
    fn probe_template_accepts_helper_override() {
        probe_template("#let task-todo(body) = ([#text(fill: red)[TODO]], [body])")
            .expect("valid override must pass");
    }

    /// A template with a syntax error is rejected — the prelude cannot make
    /// broken user code compile.
    #[test]
    fn probe_template_rejects_syntax_error() {
        let err = probe_template("#let x = =").unwrap_err();
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
        let err = probe_template("#let task-list(..items) = boom(1)").unwrap_err();
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
        let err = probe_template(tmpl).unwrap_err();
        assert!(
            err.contains("line"),
            "probe failure should carry a diagnostic: {err}"
        );
    }
}
