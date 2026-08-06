//! Markdown → Typst converter (§6.2 of SPEC.md).
//!
//! Walks the comrak AST and emits escaped Typst markup, plus any YAML
//! front-matter `title`/`author` for the template (§6.3).

use std::collections::HashMap;

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

    // Footnote definitions are rendered inline at their references —
    // `#footnote[...]` attaches the superscript marker where it is called, so
    // the definition block itself must not emit anything (§6.2).
    let mut footnote_defs: HashMap<String, String> = HashMap::new();
    collect_footnotes(root, &mut footnote_defs);

    for child in root.children() {
        if let NodeValue::FrontMatter(yaml) = &child.data().value {
            let (t, a) = parse_front_matter(yaml);
            title = t;
            author = a;
        } else {
            blocks.push(render_block_at(child, 0, &footnote_defs));
        }
    }

    // Empty document → an empty content block so compile always succeeds.
    // Footnote definitions render inline at their references and contribute
    // an empty block here, so drop empties before joining.
    let body = if blocks.is_empty() {
        "[]".to_string()
    } else {
        let non_empty: Vec<String> = blocks.into_iter().filter(|b| !b.is_empty()).collect();
        if non_empty.is_empty() {
            "[]".to_string()
        } else {
            non_empty.join("\n\n")
        }
    };
    ConvertedDocument {
        body,
        title,
        author,
    }
}

/// Walk the tree collecting footnote definitions (`name` → rendered content)
/// so references can pull their definition's content at the call site.
fn collect_footnotes(node: Node, defs: &mut HashMap<String, String>) {
    for child in node.children() {
        if let NodeValue::FootnoteDefinition(def) = &child.data().value {
            if !defs.contains_key(def.name.as_str()) {
                let content = render_block_children(child, defs);
                defs.insert(def.name.clone(), content);
            }
        }
        collect_footnotes(child, defs);
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
fn render_block_children(node: Node, footnotes: &HashMap<String, String>) -> String {
    node.children()
        .map(|child| render_block_at(child, 0, footnotes))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_block_at(node: Node, indent: usize, footnotes: &HashMap<String, String>) -> String {
    match &node.data().value {
        NodeValue::Paragraph => render_inlines(node, footnotes),
        NodeValue::CodeBlock(block) => {
            // The prelude's `code-block` owns the fenced/block styling and
            // the language fallback; Rust passes the language (or "") and the
            // source as string arguments — no backtick-fence math needed.
            let lang = block
                .info
                .split_whitespace()
                .next()
                .filter(|word| !word.is_empty())
                .unwrap_or("");
            let content = block.literal.strip_suffix('\n').unwrap_or(&block.literal);
            format!(
                "#code-block(\"{}\", \"{}\")",
                escape_typst_string(lang),
                escape_typst_string(content)
            )
        }
        NodeValue::Heading(heading) => {
            format!(
                "{} {}",
                "=".repeat(heading.level as usize),
                render_inlines(node, footnotes)
            )
        }
        NodeValue::List(list) => render_list(node, list, indent, footnotes),
        NodeValue::BlockQuote => format!("#blockquote[{}]", render_block_children(node, footnotes)),
        // The prelude's `thematic-break` owns the spacing/stroke constants.
        NodeValue::ThematicBreak => "#thematic-break()".to_string(),
        // Lossy HTML: keep the literal as plain text, escaped. Never pass
        // HTML through to Typst (§6.2).
        NodeValue::HtmlBlock(html) => escape_text(&html.literal),
        // Footnote definitions are emitted inline at their references (the
        // `#footnote[...]` marker must sit at the call site), so the
        // definition block itself contributes nothing (§6.2).
        NodeValue::FootnoteDefinition(_) => String::new(),
        // The prelude's `markdown-table` owns column sizing and the header
        // styling; Rust passes the column count plus the header cells and
        // body rows as data.
        NodeValue::Table(table) => {
            let columns = table.num_columns.max(1);
            let mut header: Vec<String> = Vec::new();
            let mut rows: Vec<String> = Vec::new();
            for row in node.children() {
                let is_header = matches!(&row.data().value, NodeValue::TableRow(true));
                let cells: Vec<String> = row
                    .children()
                    .map(|cell| format!("[{}]", render_inlines(cell, footnotes)))
                    .collect();
                if is_header {
                    header = cells;
                } else {
                    rows.push(array_literal(&cells));
                }
            }
            format!(
                "#markdown-table(columns: {columns}, header: {}, rows: {})",
                array_literal(&header),
                array_literal(&rows),
            )
        }
        _ => String::new(),
    }
}

fn render_list(
    node: Node,
    list: &comrak::nodes::NodeList,
    indent: usize,
    footnotes: &HashMap<String, String>,
) -> String {
    // Task lists become a grid via the prelude helpers (checkbox glyph +
    // content) so checked and unchecked items can mix at one level — a native
    // Typst list has a single marker for all items (§6.2).
    let has_task_items = node
        .children()
        .any(|item| matches!(item.data().value, NodeValue::TaskItem(_)));
    if has_task_items {
        return render_task_list(node, list, footnotes);
    }

    if list.list_type == comrak::nodes::ListType::Ordered {
        render_ordered_list(node, list, footnotes)
    } else {
        render_bullet_list(node, list, indent, footnotes)
    }
}

/// Ordered lists use the prelude's `ordered-list` helper (a native Typst
/// enum) so the numbering is themeable — a literal "1. " prefix would be
/// plain text that no template could style.
fn render_ordered_list(
    node: Node,
    list: &comrak::nodes::NodeList,
    footnotes: &HashMap<String, String>,
) -> String {
    let sep = if list.tight { "\n" } else { "\n\n" };
    let items: Vec<String> = node
        .children()
        .map(|item| format!("[{}]", render_item_content(item, sep, footnotes)))
        .collect();
    format!("#ordered-list({}, {})", list.start, items.join(", "))
}

/// Bullet lists stay native Typst `- ` lists so the marker is themeable via
/// `#show list`. Nested lists keep their own indentation; other blocks are
/// indented two spaces per level.
fn render_bullet_list(
    node: Node,
    list: &comrak::nodes::NodeList,
    indent: usize,
    footnotes: &HashMap<String, String>,
) -> String {
    let pad = "  ".repeat(indent);
    let pad_next = "  ".repeat(indent + 1);
    // Tight lists put every block on the next line; loose lists separate
    // blocks with a blank line (GFM semantics, mirrored in Typst).
    let sep = if list.tight { "\n" } else { "\n\n" };

    let mut lines = Vec::new();
    for item in node.children() {
        let blocks: Vec<(bool, String)> = item
            .children()
            .map(|block| {
                let is_list = matches!(block.data().value, NodeValue::List(_));
                let rendered = render_block_at(block, indent + 1, footnotes);
                (is_list, rendered)
            })
            .collect();

        let mut item_text = format!("{pad}- ");
        for (i, (is_list, rendered)) in blocks.iter().enumerate() {
            if i == 0 {
                item_text.push_str(rendered);
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

/// Render a task list as `#task-list(task-done([…]), task-todo([…]), …)`:
/// each item becomes a call to the matching prelude helper, which owns the
/// checkbox glyph and the content cell (§6.2).
fn render_task_list(
    node: Node,
    list: &comrak::nodes::NodeList,
    footnotes: &HashMap<String, String>,
) -> String {
    let sep = if list.tight { "\n" } else { "\n\n" };
    let items: Vec<String> = node
        .children()
        .map(|item| {
            let helper = match &item.data().value {
                // `sym.ballot` is ☐, `sym.ballot.check` is ☑ (U+2610/U+2611);
                // the glyph itself lives in the prelude helper.
                NodeValue::TaskItem(task) if task.symbol.is_some() => "task-done",
                NodeValue::TaskItem(_) => "task-todo",
                // A plain bullet item mixed into a task list keeps its bullet.
                _ => "task-plain",
            };
            format!("{helper}([{}])", render_item_content(item, sep, footnotes))
        })
        .collect();
    format!("#task-list({})", items.join(", "))
}

/// The blocks of a task-list item, joined for one grid cell. Nested lists
/// carry their own indentation; text is escaped by the block renderer (§6.2).
fn render_item_content(item: Node, sep: &str, footnotes: &HashMap<String, String>) -> String {
    item.children()
        .map(|block| render_block_at(block, 0, footnotes))
        .collect::<Vec<_>>()
        .join(sep)
}

/// Prefix every line of `text` with `pad`.
fn indent_lines(text: &str, pad: &str) -> String {
    text.lines()
        .map(|line| format!("{pad}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A Typst array literal from cell strings: `(a, b, c)` — with the trailing
/// comma a single-element array needs (`(x,)`); empty → `()`.
fn array_literal(cells: &[String]) -> String {
    if cells.len() == 1 {
        format!("({},)", cells[0])
    } else {
        format!("({})", cells.join(", "))
    }
}

/// Render a node's inline children into escaped Typst markup.
fn render_inlines(node: Node, footnotes: &HashMap<String, String>) -> String {
    let mut out = String::new();
    for child in node.children() {
        out.push_str(&render_inline(child, footnotes));
    }
    out
}

/// Render a single inline node.
fn render_inline(node: Node, footnotes: &HashMap<String, String>) -> String {
    match &node.data().value {
        NodeValue::Text(text) => escape_text(text),
        NodeValue::SoftBreak => " ".to_string(),
        // `\ ` (backslash + space) is a hard line break in Typst markup.
        NodeValue::LineBreak => "\\ ".to_string(),
        NodeValue::Code(code) => {
            // The prelude's `inline-code` owns the styling; the string
            // argument avoids the backtick-fence branch entirely (§6.2).
            format!("#inline-code(\"{}\")", escape_typst_string(&code.literal))
        }
        NodeValue::Strong => format!("*{}*", render_inlines(node, footnotes)),
        NodeValue::Emph => format!("_{}_", render_inlines(node, footnotes)),
        NodeValue::Strikethrough => format!("#strike[{}]", render_inlines(node, footnotes)),
        NodeValue::Link(link) => {
            format!(
                "#link(\"{}\")[{}]",
                escape_typst_string(&link.url),
                render_inlines(node, footnotes)
            )
        }
        NodeValue::Image(link) => {
            // The prelude's `figure-image` owns the default width and caption
            // placement; Rust passes the URL (data) and the alt text (content).
            format!(
                "#figure-image(\"{}\", [{}])",
                escape_typst_string(&link.url),
                render_inlines(node, footnotes)
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
        // A footnote reference is inlined at the call site — `#footnote[...]`
        // attaches the superscript marker where it is called (§6.2).
        NodeValue::FootnoteReference(reference) => match footnotes.get(&reference.name) {
            Some(content) => format!("#footnote[{content}]"),
            // Dangling reference: drop rather than emit a broken marker.
            None => String::new(),
        },
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
        assert_eq!(plain.body, "use #inline-code(\"code\") here");

        // ``a `b``  →  a code span containing a backtick; the string argument
        // needs no fence-length math.
        let backtick = md_to_typst("``a `b``", Flavor::Gfm);
        assert_eq!(backtick.body, "#inline-code(\"a `b\")");

        // A literal with a backtick *and* quotes/backslashes → escaped string.
        let quoted = md_to_typst("```a `b \"c\\```", Flavor::Gfm);
        assert_eq!(quoted.body, "#inline-code(\"a `b \\\"c\\\\\")");
    }

    #[test]
    fn maps_code_blocks() {
        let rust = md_to_typst("```rust\nfn main() {}\n```", Flavor::Gfm);
        assert_eq!(rust.body, "#code-block(\"rust\", \"fn main() {}\")");

        // No language → the empty string; the prelude owns the fallback.
        let no_lang = md_to_typst("```\nplain\n```", Flavor::Gfm);
        assert_eq!(no_lang.body, "#code-block(\"\", \"plain\")");

        // A literal containing a backtick run needs no longer fence.
        let backticks = md_to_typst("```\na ``` b\n```", Flavor::Gfm);
        assert_eq!(backticks.body, "#code-block(\"\", \"a ``` b\")");
    }

    #[test]
    fn maps_links_and_images() {
        let doc = md_to_typst(
            "[label](https://example.com) and ![alt](img.png)",
            Flavor::Gfm,
        );
        assert_eq!(
            doc.body,
            "#link(\"https://example.com\")[label] and #figure-image(\"img.png\", [alt])"
        );

        // A quote in the URL is escaped inside the Typst string literal.
        let quoted = md_to_typst("[x](<https://a.com/\"q\">)", Flavor::Gfm);
        assert_eq!(quoted.body, "#link(\"https://a.com/\\\"q\\\"\")[x]");
    }

    #[test]
    fn maps_bullet_and_ordered_lists() {
        // Bullet lists stay native Typst `- ` lists (themeable via #show list).
        let bullet = md_to_typst("- a\n- b", Flavor::Gfm);
        assert_eq!(bullet.body, "- a\n- b");

        // Ordered lists use the prelude helper (a native enum) so the
        // numbering is themeable; the markdown start value is preserved.
        let ordered = md_to_typst("1. a\n2. b", Flavor::Gfm);
        assert_eq!(ordered.body, "#ordered-list(1, [a], [b])");

        let starts_at_three = md_to_typst("3. a\n4. b", Flavor::Gfm);
        assert_eq!(starts_at_three.body, "#ordered-list(3, [a], [b])");
    }

    #[test]
    fn maps_nested_lists_with_indentation() {
        let nested = md_to_typst("- a\n  - b", Flavor::Gfm);
        assert_eq!(nested.body, "- a\n  - b");
    }

    #[test]
    fn maps_task_list_items() {
        // Task lists become calls to the prelude helpers, each owning the
        // checkbox glyph and content cell (§6.2).
        let doc = md_to_typst("- [x] done\n- [ ] todo", Flavor::Gfm);
        assert_eq!(doc.body, "#task-list(task-done([done]), task-todo([todo]))");
    }

    #[test]
    fn maps_plain_bullet_item_mixed_into_a_task_list() {
        // GFM allows a non-checkbox bullet item in the same list; it gets the
        // `task-plain` helper (default bullet glyph in the checkbox column).
        let doc = md_to_typst("- [x] done\n- plain item", Flavor::Gfm);
        assert_eq!(
            doc.body,
            "#task-list(task-done([done]), task-plain([plain item]))"
        );
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
            "#markdown-table(columns: 2, header: ([a], [b]), rows: (([1], [2]),))"
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
        assert_eq!(hr.body, "a\n\n#thematic-break()\n\nb");
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
        // A footnote reference is inlined at its call site — `#footnote[...]`
        // puts the superscript marker where it is called (§6.2); the
        // definition block itself is dropped from the flow.
        let custom = md_to_typst("text[^1]\n\n[^1]: the note", Flavor::Custom);
        assert_eq!(custom.body, "text#footnote[the note]");

        let math = md_to_typst("Inline $x^2$ and display $$y$$.", Flavor::Custom);
        assert_eq!(math.body, "Inline $x^2$ and display $ y $.");

        // Without the extension the syntax is ordinary (escaped) text.
        let gfm = md_to_typst("text[^1] and $x^2$", Flavor::Gfm);
        assert_eq!(gfm.body, "text\\[^1\\] and \\$x^2\\$");
    }

    #[test]
    fn maps_footnote_reference_inside_a_list_item() {
        // The marker must stay inline in the bullet item, not drift to the
        // definition at the end of the document.
        let custom = md_to_typst(
            "- bullet [^1]\n- another item\n\n[^1]: the note",
            Flavor::Custom,
        );
        assert_eq!(custom.body, "- bullet #footnote[the note]\n- another item");
    }

    #[test]
    fn leaves_unresolved_footnote_syntax_as_literal_text() {
        // comrak only recognizes `[^1]` as a footnote reference when a
        // matching definition exists; otherwise it is ordinary escaped text.
        let custom = md_to_typst("text[^1]", Flavor::Custom);
        assert_eq!(custom.body, "text\\[^1\\]");
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
        assert_eq!(doc.body, "#code-block(\"\", \"let x = 1;\")");

        let blank = md_to_typst("    a\n\n    b\n", Flavor::Gfm);
        assert_eq!(blank.body, "#code-block(\"\", \"a\n\nb\")");
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
