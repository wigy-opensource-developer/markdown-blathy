//! Markdown → Typst converter (§6.2 of SPEC.md).
//!
//! Walks the comrak AST and emits escaped Typst markup, plus any YAML
//! front-matter `title`/`author` for the template (§6.3).

use comrak::nodes::{Node, NodeValue};
use comrak::{parse_document, Arena};

use crate::markdown::{comrak_options, escape::escape_text, escape::escape_typst_string};
use crate::options::Flavor;

/// Typst body plus front-matter metadata (empty doc → body `[]`).
pub struct ConvertedDocument {
    pub body: String,
    pub title: Option<String>,
    pub author: Option<String>,
}

/// Convert Markdown to Typst markup for the given flavor.
///
/// Infallible: comrak's `parse_document` cannot fail, and the v1 front-matter
/// scan (plain line parse) cannot either. `SPEC.md` originally specified a
/// `Result`; it was dropped because there is nothing to fail with.
pub fn md_to_typst(markdown: &str, flavor: Flavor) -> ConvertedDocument {
    let arena = Arena::new();
    let opts = comrak_options(flavor);
    let root = parse_document(&arena, markdown, &opts);

    let mut title = None;
    let mut author = None;
    let mut blocks: Vec<String> = Vec::new();
    for child in root.children() {
        if let NodeValue::FrontMatter(yaml) = &child.data().value {
            let (t, a) = parse_front_matter(yaml);
            title = t;
            author = a;
        } else {
            blocks.push(render_block_at(child, 0));
        }
    }

    // Empty document → an empty content block so compile always succeeds.
    let body = if blocks.is_empty() {
        "[]".to_string()
    } else {
        blocks.join("\n\n")
    };
    ConvertedDocument {
        body,
        title,
        author,
    }
}

/// Plain line-scan for `title:` / `author:` (v1 — no YAML crate). Splits on
/// the first `: ` and trims surrounding quotes. Ignores everything else.
fn parse_front_matter(yaml: &str) -> (Option<String>, Option<String>) {
    let mut title = None;
    let mut author = None;
    for line in yaml.lines() {
        if title.is_none() {
            title = yaml_field(line, "title");
        }
        if author.is_none() {
            author = yaml_field(line, "author");
        }
        if title.is_some() && author.is_some() {
            break;
        }
    }
    (title, author)
}

/// Read one `key: value` field from a YAML line, if present.
fn yaml_field(line: &str, key: &str) -> Option<String> {
    line.split_once(": ")
        .filter(|(k, _)| k.trim() == key)
        .map(|(_, value)| clean_yaml_value(value))
}

/// Trim whitespace and strip a pair of surrounding double quotes.
fn clean_yaml_value(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// Render a node's block children joined with blank lines (used by
/// `#blockquote[…]` and `#footnote[…]` containers).
fn render_block_children(node: Node) -> String {
    node.children()
        .map(|child| render_block_at(child, 0))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_block_at(node: Node, indent: usize) -> String {
    match &node.data().value {
        NodeValue::Paragraph => render_inlines(node),
        NodeValue::CodeBlock(block) => {
            let lang = block
                .info
                .split_whitespace()
                .next()
                .filter(|word| !word.is_empty())
                .unwrap_or("text");
            // A fence longer than any backtick run in the content is always
            // unambiguous (min 3, as in CommonMark).
            let content = block.literal.strip_suffix('\n').unwrap_or(&block.literal);
            let fence_len = (longest_backtick_run(content) + 1).max(3);
            let fence = "`".repeat(fence_len);
            format!("{fence}{lang}\n{content}\n{fence}")
        }
        NodeValue::Heading(heading) => {
            format!(
                "{} {}",
                "=".repeat(heading.level as usize),
                render_inlines(node)
            )
        }
        NodeValue::List(list) => render_list(node, list, indent),
        NodeValue::BlockQuote => format!("#blockquote[{}]", render_block_children(node)),
        NodeValue::ThematicBreak => {
            "#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)".to_string()
        }
        // Lossy HTML: keep the literal as plain text, escaped. Never pass
        // HTML through to Typst (§6.2).
        NodeValue::HtmlBlock(html) => escape_text(&html.literal),
        NodeValue::FootnoteDefinition(_) => format!("#footnote[{}]", render_block_children(node)),
        NodeValue::Table(table) => {
            let columns = vec!["auto"; table.num_columns.max(1)].join(", ");
            let mut cells: Vec<String> = Vec::new();
            for row in node.children() {
                let is_header = matches!(&row.data().value, NodeValue::TableRow(true));
                for cell in row.children() {
                    let content = render_inlines(cell);
                    // Header cells are *bold*: emit the strong markup *inside*
                    // the content block. `*[a]` would be invalid here — table
                    // arguments are parsed in code mode, where `*` is
                    // multiplication, not emphasis.
                    cells.push(if is_header {
                        format!("[*{content}*]")
                    } else {
                        format!("[{content}]")
                    });
                }
            }
            format!("#table(columns: ({columns}), {})", cells.join(", "))
        }
        _ => String::new(),
    }
}

fn render_list(node: Node, list: &comrak::nodes::NodeList, indent: usize) -> String {
    let pad = "  ".repeat(indent);
    let pad_next = "  ".repeat(indent + 1);
    // Tight lists put every block on the next line; loose lists separate
    // blocks with a blank line (GFM semantics, mirrored in Typst).
    let sep = if list.tight { "\n" } else { "\n\n" };

    let mut lines = Vec::new();
    let mut ordinal = list.start;
    for item in node.children() {
        let marker = match list.list_type {
            comrak::nodes::ListType::Bullet => "- ".to_string(),
            comrak::nodes::ListType::Ordered => format!("{ordinal}. "),
        };
        ordinal += 1;

        // Task-list items: the item node's value is TaskItem (§6.2); the
        // checkbox becomes #task(checked) right after the marker.
        let task = match &item.data().value {
            NodeValue::TaskItem(task) => format!("#task({}) ", task.symbol.is_some()),
            _ => String::new(),
        };

        let blocks: Vec<(bool, String)> = item
            .children()
            .map(|block| {
                let is_list = matches!(block.data().value, NodeValue::List(_));
                let rendered = render_block_at(block, indent + 1);
                (is_list, rendered)
            })
            .collect();

        let mut item_text = String::new();
        for (i, (is_list, rendered)) in blocks.iter().enumerate() {
            if i == 0 {
                item_text.push_str(&format!("{pad}{marker}{task}{rendered}"));
            } else {
                item_text.push_str(sep);
                if *is_list {
                    // Nested lists already carry their own indentation.
                    item_text.push_str(rendered);
                } else {
                    item_text.push_str(&indent_lines(rendered, &pad_next));
                }
            }
        }
        lines.push(item_text);
    }
    lines.join("\n")
}

/// Prefix every line of `text` with `pad`.
fn indent_lines(text: &str, pad: &str) -> String {
    text.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Longest run of consecutive backticks in `text` (0 if none).
fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for c in text.chars() {
        if c == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }
    longest
}

/// Render a node's inline children into escaped Typst markup.
fn render_inlines(node: Node) -> String {
    let mut out = String::new();
    for child in node.children() {
        out.push_str(&render_inline(child));
    }
    out
}

/// Render a single inline node.
fn render_inline(node: Node) -> String {
    match &node.data().value {
        NodeValue::Text(text) => escape_text(text),
        NodeValue::SoftBreak => " ".to_string(),
        // `\ ` (backslash + space) is a hard line break in Typst markup.
        NodeValue::LineBreak => "\\ ".to_string(),
        NodeValue::Code(code) => {
            if code.literal.contains('`') {
                format!("#raw(\"{}\")", escape_typst_string(&code.literal))
            } else {
                format!("`{}`", code.literal)
            }
        }
        NodeValue::Strong => format!("*{}*", render_inlines(node)),
        NodeValue::Emph => format!("_{}_", render_inlines(node)),
        NodeValue::Strikethrough => format!("#strike[{}]", render_inlines(node)),
        NodeValue::Link(link) => {
            format!(
                "#link(\"{}\")[{}]",
                escape_typst_string(&link.url),
                render_inlines(node)
            )
        }
        NodeValue::Image(link) => {
            format!(
                "#figure(image(\"{}\", width: 80%), caption: [{}])",
                escape_typst_string(&link.url),
                render_inlines(node)
            )
        }
        NodeValue::HtmlInline(html) => escape_text(html),
        // Passthrough: LaTeX macros are not converted in v1 (§6.2).
        NodeValue::Math(math) => {
            if math.display_math {
                format!("$ {} $", math.literal)
            } else {
                format!("${}$", math.literal)
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::md_to_typst;
    use crate::options::Flavor;

    #[test]
    fn empty_input_returns_empty_body() {
        for input in ["", "   ", "\n\n"] {
            let doc = md_to_typst(input, Flavor::Gfm);
            assert_eq!(doc.body, "[]", "input: {input:?}");
            assert_eq!(doc.title, None);
            assert_eq!(doc.author, None);
        }
    }

    #[test]
    fn maps_headings_to_typst_levels() {
        let doc = md_to_typst("# Title\n\n## Sub\n\n### Third", Flavor::Gfm);
        assert_eq!(doc.body, "= Title\n\n== Sub\n\n=== Third");
    }

    #[test]
    fn maps_emphasis_strong_and_strikethrough() {
        let doc = md_to_typst("**bold** and *em* and ~~strike~~", Flavor::Gfm);
        assert_eq!(doc.body, "*bold* and _em_ and #strike[strike]");
    }

    #[test]
    fn soft_breaks_become_spaces_hard_breaks_escaped() {
        let soft = md_to_typst("line one\nline two", Flavor::Gfm);
        assert_eq!(soft.body, "line one line two");

        let hard = md_to_typst("line one\\\nline two", Flavor::Gfm);
        assert_eq!(hard.body, "line one\\ line two");
    }

    #[test]
    fn maps_inline_code_and_backtick_containing_code() {
        let plain = md_to_typst("use `code` here", Flavor::Gfm);
        assert_eq!(plain.body, "use `code` here");

        // ``a `b``  →  code span containing a backtick → must use #raw("…").
        let backtick = md_to_typst("``a `b``", Flavor::Gfm);
        assert_eq!(backtick.body, "#raw(\"a `b\")");

        // A literal with a backtick *and* quotes/backslashes → #raw("…")
        // with the string content escaped.
        let quoted = md_to_typst("```a `b \"c\\```", Flavor::Gfm);
        assert_eq!(quoted.body, "#raw(\"a `b \\\"c\\\\\")");
    }

    #[test]
    fn maps_code_blocks() {
        let rust = md_to_typst("```rust\nfn main() {}\n```", Flavor::Gfm);
        assert_eq!(rust.body, "```rust\nfn main() {}\n```");

        let no_lang = md_to_typst("```\nplain\n```", Flavor::Gfm);
        assert_eq!(no_lang.body, "```text\nplain\n```");

        // A literal containing a backtick run needs a longer fence.
        let backticks = md_to_typst("```\na ``` b\n```", Flavor::Gfm);
        assert_eq!(backticks.body, "````text\na ``` b\n````");
    }

    #[test]
    fn maps_links_and_images() {
        let doc = md_to_typst(
            "[label](https://example.com) and ![alt](img.png)",
            Flavor::Gfm,
        );
        assert_eq!(
            doc.body,
            "#link(\"https://example.com\")[label] and #figure(image(\"img.png\", width: 80%), caption: [alt])"
        );

        // A quote in the URL is escaped inside the Typst string literal.
        let quoted = md_to_typst("[x](<https://a.com/\"q\">)", Flavor::Gfm);
        assert_eq!(quoted.body, "#link(\"https://a.com/\\\"q\\\"\")[x]");
    }

    #[test]
    fn maps_bullet_and_ordered_lists() {
        let bullet = md_to_typst("- a\n- b", Flavor::Gfm);
        assert_eq!(bullet.body, "- a\n- b");

        let ordered = md_to_typst("1. a\n2. b", Flavor::Gfm);
        assert_eq!(ordered.body, "1. a\n2. b");
    }

    #[test]
    fn maps_nested_lists_with_indentation() {
        let nested = md_to_typst("- a\n  - b", Flavor::Gfm);
        assert_eq!(nested.body, "- a\n  - b");
    }

    #[test]
    fn maps_task_list_items() {
        let doc = md_to_typst("- [x] done\n- [ ] todo", Flavor::Gfm);
        assert_eq!(doc.body, "- #task(true) done\n- #task(false) todo");
    }

    #[test]
    fn loose_list_items_keep_paragraph_breaks() {
        let doc = md_to_typst("- a\n\n  continued", Flavor::Gfm);
        assert_eq!(doc.body, "- a\n\n  continued");
    }

    #[test]
    fn maps_gfm_tables_only_in_gfm_and_custom_flavors() {
        let gfm = md_to_typst("| a | b |\n|---|---|\n| 1 | 2 |", Flavor::Gfm);
        assert_eq!(
            gfm.body,
            "#table(columns: (auto, auto), [*a*], [*b*], [1], [2])"
        );

        // CommonMark has no table extension: the same source is paragraphs.
        let commonmark = md_to_typst("| a | b |\n|---|---|\n| 1 | 2 |", Flavor::Commonmark);
        assert!(commonmark.body.contains("| a | b |"), "{}", commonmark.body);
    }

    #[test]
    fn maps_blockquotes_and_thematic_breaks() {
        let quote = md_to_typst("> quoted text", Flavor::Gfm);
        assert_eq!(quote.body, "#blockquote[quoted text]");

        let hr = md_to_typst("a\n\n---\n\nb", Flavor::Gfm);
        assert_eq!(
            hr.body,
            "a\n\n#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)\n\nb"
        );
    }

    #[test]
    fn strips_html_to_escaped_plain_text() {
        // Inside an HTML *block*, content is not parsed as markdown, so the
        // whole literal is emitted as escaped plain text.
        let block = md_to_typst("<div>\n<p>*hi*</p>\n</div>", Flavor::Gfm);
        assert_eq!(block.body, "<div>\n<p>\\*hi\\*</p>\n</div>");

        // Inline HTML tags pass through; the surrounding markdown is parsed.
        let inline = md_to_typst("a <span>*x*</span> b", Flavor::Gfm);
        assert_eq!(inline.body, "a <span>_x_</span> b");
    }

    #[test]
    fn maps_footnotes_and_math_in_custom_flavor_only() {
        // v1: the definition's content becomes the #footnote (at its own
        // position); the inline reference marker is dropped (§6.2).
        let custom = md_to_typst("text[^1]\n\n[^1]: the note", Flavor::Custom);
        assert_eq!(custom.body, "text\n\n#footnote[the note]");

        let math = md_to_typst("Inline $x^2$ and display $$y$$.", Flavor::Custom);
        assert_eq!(math.body, "Inline $x^2$ and display $ y $.");

        // Without the extension the syntax is ordinary (escaped) text.
        let gfm = md_to_typst("text[^1] and $x^2$", Flavor::Gfm);
        assert_eq!(gfm.body, "text\\[^1\\] and \\$x^2\\$");
    }

    #[test]
    fn parses_front_matter_title_and_author() {
        let doc = md_to_typst(
            "---\ntitle: \"My Doc\"\nauthor: \"Ada\"\n---\n\n# Body",
            Flavor::Custom,
        );
        assert_eq!(doc.title.as_deref(), Some("My Doc"));
        assert_eq!(doc.author.as_deref(), Some("Ada"));
        assert_eq!(doc.body, "= Body");
    }

    #[test]
    fn front_matter_is_optional_and_unquoted_values_work() {
        let unquoted = md_to_typst("---\ntitle: Plain Doc\n---\n\nBody", Flavor::Custom);
        assert_eq!(unquoted.title.as_deref(), Some("Plain Doc"));
        assert_eq!(unquoted.author, None);

        let none = md_to_typst("# No front matter", Flavor::Custom);
        assert_eq!(none.title, None);
        assert_eq!(none.author, None);
    }

    #[test]
    fn maps_indented_code_blocks() {
        let doc = md_to_typst("    let x = 1;\n", Flavor::Gfm);
        assert_eq!(doc.body, "```text\nlet x = 1;\n```");

        let blank = md_to_typst("    a\n\n    b\n", Flavor::Gfm);
        assert_eq!(blank.body, "```text\na\n\nb\n```");
    }

    #[test]
    fn maps_autolinks_without_label() {
        // GFM autolink: the URL is also the link text.
        let doc = md_to_typst("see <https://example.com> now", Flavor::Gfm);
        assert_eq!(
            doc.body,
            "see #link(\"https://example.com\")[https://example.com] now"
        );
    }

    #[test]
    fn front_matter_ignores_other_keys() {
        let doc = md_to_typst(
            "---\ntitle: \"T\"\ndate: \"2026-01-01\"\ntags: [a]\nauthor: \"A\"\n---\n\nBody",
            Flavor::Custom,
        );
        assert_eq!(doc.title.as_deref(), Some("T"));
        assert_eq!(doc.author.as_deref(), Some("A"));
    }
}
