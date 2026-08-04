//! Typst template composition (§6.3 of SPEC.md).
//!
//! Pure Rust — no `tauri` imports. The template source (embedded by default,
//! or read from a user path by the command layer) is concatenated with an
//! `#article(...)[body]` invocation carrying the render options and the
//! already-escaped markdown body.

use crate::markdown::convert::md_to_typst;
use crate::options::{RenderOptions, TextAlign};

/// The embedded default template (`templates/default.typ`).
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

#[cfg(test)]
mod tests {
    use super::{build_source, compose, TemplateArgs, DEFAULT_TEMPLATE};
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
}
