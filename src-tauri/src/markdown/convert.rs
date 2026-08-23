//! Markdown → Typst converter (§6.2 of SPEC.md).
//!
//! Walks the comrak AST and emits escaped Typst markup, plus any YAML
//! front-matter `title`/`author` for the template (§6.3).

use std::collections::{HashMap, HashSet};

use comrak::arena_tree::NodeEdge;
use comrak::nodes::{Node, NodeValue};
use comrak::{parse_document, Arena};
use serde::Serialize;

use crate::markdown::{
    comrak_options, escape::escape_text, escape::escape_text_into, escape::escape_typst_string,
    escape::escape_typst_string_into, frontmatter,
};
use crate::options::{FeatureFlags, Flavor, HeadingIds};

/// A single entry in the table of contents: heading level, rendered text,
/// and the slug that the heading's label resolves to.
#[derive(Debug, Clone, Serialize)]
pub struct TocItem {
    /// 1–6, matching the Markdown heading level.
    pub level: u8,
    /// The heading's rendered inline text (escaped for Typst).
    pub text: String,
    /// The slug used in the `#label("…")` call (empty when the heading
    /// carries no ID — the prelude helper skips such entries).
    pub slug: String,
}

/// Typst body plus front-matter metadata (empty doc → body `[]`) and the
/// effective document settings, so the frontend can display/edit them
/// without parsing YAML itself (§6.2).
pub struct ConvertedDocument {
    pub body: String,
    pub title: Option<String>,
    pub author: Option<String>,
    /// Non-fatal warnings (front matter first, then rendering).
    pub warnings: Vec<String>,
    /// The effective flavor (front matter overrides the toolbar).
    pub flavor: Flavor,
    /// The effective extension flags (flavor preset + `ext_*` overrides).
    pub features: FeatureFlags,
    /// The effective auto heading level (0 = disabled).
    pub auto_heading_level: u8,
    /// Headings collected for the `[[_TOC_]]` marker (Custom flavor).
    pub toc: Vec<TocItem>,
}

/// The effective document settings, serialized to the frontend so it can
/// display (and later edit) them without parsing YAML itself (§6.2). A
/// deliberately small view of `ConvertedDocument` — no body, no warnings —
/// so it is cheap to clone into the IPC response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub flavor: Flavor,
    pub features: FeatureFlags,
    pub auto_heading_level: u8,
    /// Headings collected for the `[[_TOC_]]` marker (Custom flavor).
    #[serde(default)]
    pub toc: Vec<TocItem>,
}

/// Convert Markdown to Typst markup.
///
/// The document's front matter (parsed in every flavor) overrides the
/// toolbar `base_flavor` / `base_heading_ids`. Infallible: comrak's
/// `parse_document` cannot fail and the front-matter line scan cannot either.
pub fn md_to_typst(
    markdown: &str,
    base_flavor: Flavor,
    base_heading_ids: &HeadingIds,
) -> ConvertedDocument {
    // Front matter is always the first block; scan it before rendering so the
    // document's settings take effect for every heading, including ones
    // inside footnote definitions (§6.2). The scan may hand back a patched
    // source (a stray leading `---` stripped) — the original is untouched.
    let scan = frontmatter::scan(markdown);

    // Move the parsed fields out of the settings (no clones).
    let (title, author, fm_flavor, fm_auto_level, ext_overrides, fm_warnings) = match scan.settings
    {
        Some(s) => (
            s.title,
            s.author,
            s.flavor,
            s.auto_heading_level,
            s.ext_overrides,
            s.warnings,
        ),
        None => (None, None, None, None, Vec::new(), Vec::new()),
    };

    // Effective settings: front matter beats the toolbar.
    let flavor = fm_flavor.unwrap_or(base_flavor);
    let features = FeatureFlags::from_preset(flavor, &ext_overrides);
    let heading_ids = fm_auto_level
        .map(|max_level| HeadingIds { max_level })
        .unwrap_or(*base_heading_ids);

    // Pre-process: replace `[[_TOC_]]` (GitLab convention) with an HTML
    // comment so comrak parses it as an HtmlBlock — gated on `page_toc`.
    let source = if features.page_toc {
        scan.source
            .replace("[[_TOC_]]", "<!-- [[_TOC_]] -->")
            .to_string()
    } else {
        scan.source.to_string()
    };

    let arena = Arena::new();
    let opts = comrak_options(&features);
    let root = parse_document(&arena, &source, &opts);

    let mut renderer = Renderer::new(heading_ids, features.page_break);
    // Footnote definitions are rendered inline at their references —
    // `#footnote[...]` attaches the superscript marker where it is called, so
    // the definition block itself must not emit anything (§6.2). Collect them
    // up front so references resolve anywhere in the document.
    renderer.collect_footnotes(root);
    // Headings are collected up front so the `[[_TOC_]]` marker can emit a
    // `#markdown-toc(...)` call with all the data (§6.2).
    renderer.collect_headings(root);

    let mut blocks: Vec<String> = Vec::new();
    for child in root.children() {
        // The front matter was parsed above (before rendering); it
        // contributes nothing to the body.
        if !matches!(&child.data().value, NodeValue::FrontMatter(_)) {
            blocks.push(renderer.render_block_at(child, 0));
        }
    }

    // Front-matter warnings first, then rendering warnings (§3 order).
    let mut warnings = scan.warnings;
    warnings.extend(fm_warnings);
    warnings.extend(renderer.warnings);

    ConvertedDocument {
        body: join_blocks(blocks),
        title,
        author,
        warnings,
        flavor,
        features,
        auto_heading_level: heading_ids.max_level,
        toc: renderer.toc_items,
    }
}

/// Join rendered blocks with blank lines, dropping empties (footnote
/// definitions render inline at their references and contribute an empty
/// block). An empty document becomes an empty content block so compile
/// always succeeds.
fn join_blocks(blocks: Vec<String>) -> String {
    let non_empty: Vec<String> = blocks.into_iter().filter(|b| !b.is_empty()).collect();
    if non_empty.is_empty() {
        "[]".to_string()
    } else {
        non_empty.join("\n\n")
    }
}

/// True when an HTML block is exactly the pandoc-convention page-break
/// marker `<!-- pagebreak -->` — whitespace inside the comment is
/// tolerated, the marker text itself is case-sensitive. Anything else
/// (other comments, extra content) stays lossy text.
fn is_page_break_marker(literal: &str) -> bool {
    literal
        .trim()
        .strip_prefix("<!--")
        .and_then(|rest| rest.strip_suffix("-->"))
        .is_some_and(|inner| inner.trim() == "pagebreak")
}

/// Per-document render state for one `md_to_typst` pass.
///
/// Collects everything the block/inline renderers share — the footnote
/// definitions, the effective heading-ID settings, the emitted-ID set, and
/// the warning list — so each render method takes only the AST node it
/// renders instead of threading four parameters through every call.
struct Renderer {
    /// Footnote definitions (`name` → rendered content), collected up front
    /// so references can pull their definition's content at the call site.
    footnotes: HashMap<String, String>,
    /// Effective heading-ID settings: the document's front-matter
    /// `autoHeadingLevel` overrides the toolbar option (§6.2).
    heading_ids: HeadingIds,
    /// Heading IDs already emitted (manual and auto share one namespace):
    /// the first occurrence wins, later duplicates are dropped with a
    /// warning.
    seen_ids: HashSet<String>,
    /// Non-fatal warnings (e.g. duplicate heading IDs).
    warnings: Vec<String>,
    /// Extension (§4): honor the pandoc-convention `<!-- pagebreak -->` HTML
    /// block as a page break.
    page_breaks: bool,
    /// Headings collected for the `[[_TOC_]]` marker (Custom flavor).
    toc_items: Vec<TocItem>,
}

impl Renderer {
    fn new(heading_ids: HeadingIds, page_breaks: bool) -> Self {
        Self {
            footnotes: HashMap::new(),
            heading_ids,
            seen_ids: HashSet::new(),
            warnings: Vec::new(),
            page_breaks,
            toc_items: Vec::new(),
        }
    }

    /// Walk the tree collecting headings (`level`, rendered text, slug) so the
    /// `[[_TOC_]]` marker can emit a `#markdown-toc(...)` call with all the
    /// data (§6.2). Iterative to match `collect_footnotes`'s stack-safety.
    fn collect_headings(&mut self, root: Node) {
        for node in root.descendants() {
            if let NodeValue::Heading(heading) = &node.data().value {
                let level = heading.level;
                let text = self.render_inlines(node);
                // Compute the slug without side effects — just the ID string,
                // no seen_ids mutation, no warnings. The block renderer will
                // do the full processing (uniqueness, warnings, label suffix).
                let slug = heading_slug(node, &self.heading_ids);
                self.toc_items.push(TocItem { level, text, slug });
            }
        }
    }

    /// Walk the tree collecting footnote definitions (`name` → rendered
    /// content) so references can pull their definition's content at the
    /// call site.
    ///
    /// Iterative on purpose: comrak caps list nesting but not blockquote
    /// nesting, so the AST depth is unbounded and a recursive walk
    /// overflows the stack on a contrived document (~10 KB of `> `
    /// markers). `descendants()` is comrak's own iterative pre-order
    /// traversal — the same visit order as the recursive walk, so the
    /// first definition of a duplicate name still wins, and heading IDs
    /// inside definitions register in the same sequence.
    fn collect_footnotes(&mut self, root: Node) {
        for node in root.descendants() {
            if let NodeValue::FootnoteDefinition(def) = &node.data().value {
                if !self.footnotes.contains_key(def.name.as_str()) {
                    let content = self.render_block_children(node);
                    self.footnotes.insert(def.name.clone(), content);
                }
            }
        }
    }

    /// Render a node's block children joined with blank lines (used by
    /// `#blockquote[…]` and `#footnote[…]` containers).
    fn render_block_children(&mut self, node: Node) -> String {
        node.children()
            .map(|child| self.render_block_at(child, 0))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn render_block_at(&mut self, node: Node, indent: usize) -> String {
        match &node.data().value {
            NodeValue::Paragraph => self.render_inlines(node),
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
                let prefix = "=".repeat(heading.level as usize);
                let text = self.render_inlines(node);
                let suffix = self.heading_label_suffix(node);
                format!("{prefix} {text}{suffix}")
            }
            NodeValue::List(list) => self.render_list(node, list, indent),
            NodeValue::BlockQuote => {
                format!("#blockquote[{}]", self.render_block_children(node))
            }
            // The prelude's `thematic-break` owns the spacing/stroke constants.
            NodeValue::ThematicBreak => "#thematic-break()".to_string(),
            // Lossy HTML: keep the literal as plain text, escaped. Never pass
            // HTML through to Typst (§6.2) — except:
            // - the pandoc-convention `<!-- pagebreak -->` marker, a
            //   whole-block page break in Custom flavor (§4),
            // - the GitLab-convention `[[_TOC_]]` marker, which becomes a
            //   `#markdown-toc((...))` call with all collected headings
            //   (Custom flavor only, §6.2).
            NodeValue::HtmlBlock(html) => {
                if self.page_breaks && is_page_break_marker(&html.literal) {
                    "#pagebreak()".to_string()
                } else if html.literal.trim() == "<!-- [[_TOC_]] -->" {
                    // The prelude's `markdown-toc` owns the layout; Rust
                    // passes the items as a list of `(level, text, slug)`
                    // tuples — uses Typst tuple syntax `(...)` not content
                    // brackets `[...]` so the template can iterate.
                    let items: Vec<String> = self
                        .toc_items
                        .iter()
                        .filter(|item| !item.slug.is_empty())
                        .map(|item| {
                            format!(
                                "({}, [{}], \"{}\")",
                                item.level,
                                escape_text(&item.text),
                                escape_typst_string(&item.slug)
                            )
                        })
                        .collect();
                    format!("#markdown-toc(({},))", items.join(", "))
                } else {
                    escape_text(&html.literal)
                }
            }
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
                        .map(|cell| format!("[{}]", self.render_inlines(cell)))
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
            // The prelude's `definition-list` lays out (term, details) pairs;
            // Rust passes each item as a `(term, details)` content tuple (§6.2).
            NodeValue::DescriptionList => {
                let mut items: Vec<String> = Vec::new();
                for item in node.children() {
                    let mut term = None;
                    let mut details = None;
                    for child in item.children() {
                        match &child.data().value {
                            // comrak moves the term's paragraph in as a child,
                            // so the term holds blocks, not bare inlines.
                            NodeValue::DescriptionTerm => {
                                term = Some(self.render_block_children(child));
                            }
                            NodeValue::DescriptionDetails => {
                                details = Some(self.render_block_children(child));
                            }
                            _ => {}
                        }
                    }
                    if let (Some(t), Some(d)) = (term, details) {
                        items.push(format!("([{}], [{}])", t, d));
                    }
                }
                format!("#definition-list({})", items.join(", "))
            }
            _ => String::new(),
        }
    }

    fn render_list(&mut self, node: Node, list: &comrak::nodes::NodeList, indent: usize) -> String {
        // Task lists become a grid via the prelude helpers (checkbox glyph +
        // content) so checked and unchecked items can mix at one level — a native
        // Typst list has a single marker for all items (§6.2).
        let has_task_items = node
            .children()
            .any(|item| matches!(item.data().value, NodeValue::TaskItem(_)));
        if has_task_items {
            return self.render_task_list(node, list);
        }

        if list.list_type == comrak::nodes::ListType::Ordered {
            self.render_ordered_list(node, list)
        } else {
            self.render_bullet_list(node, list, indent)
        }
    }

    /// Ordered lists use the prelude's `ordered-list` helper (a native Typst
    /// enum) so the numbering is themeable — a literal "1. " prefix would be
    /// plain text that no template could style.
    fn render_ordered_list(&mut self, node: Node, list: &comrak::nodes::NodeList) -> String {
        let sep = if list.tight { "\n" } else { "\n\n" };
        let items: Vec<String> = node
            .children()
            .map(|item| format!("[{}]", self.render_item_content(item, sep)))
            .collect();
        format!("#ordered-list({}, {})", list.start, items.join(", "))
    }

    /// Bullet lists stay native Typst `- ` lists so the marker is themeable via
    /// `#show list`. Nested lists keep their own indentation; other blocks are
    /// indented two spaces per level.
    fn render_bullet_list(
        &mut self,
        node: Node,
        list: &comrak::nodes::NodeList,
        indent: usize,
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
                    let rendered = self.render_block_at(block, indent + 1);
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
    fn render_task_list(&mut self, node: Node, list: &comrak::nodes::NodeList) -> String {
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
                format!("{helper}([{}])", self.render_item_content(item, sep))
            })
            .collect();
        format!("#task-list({})", items.join(", "))
    }

    /// The blocks of a task-list item, joined for one grid cell. Nested lists
    /// carry their own indentation; text is escaped by the block renderer (§6.2).
    fn render_item_content(&mut self, item: Node, sep: &str) -> String {
        item.children()
            .map(|block| self.render_block_at(block, 0))
            .collect::<Vec<_>>()
            .join(sep)
    }

    /// The heading's rendered Typst label suffix (` #label("…")`), or an
    /// empty string when the heading carries no ID.
    ///
    /// A manual `{#id}` (from comrak's `attrs.id`) is the author's explicit
    /// intent: it is always honored, at every level. `heading_ids.max_level`
    /// limits AUTO-slug generation only (0 = no auto-slugs at all).
    ///
    /// All IDs (manual and auto) share one uniqueness namespace: Typst
    /// tolerates duplicate labels at compile time, but any reference to a
    /// duplicate label fails — so the first occurrence wins and later
    /// duplicates are dropped with a warning.
    fn heading_label_suffix(&mut self, node: Node) -> String {
        let NodeValue::Heading(heading) = &node.data().value else {
            return String::new();
        };

        // Manual `{#custom-id}` wins over the auto slug and bypasses the cap.
        let manual_id = node.data().attrs.as_ref().and_then(|a| a.id.clone());

        let (id, is_manual) = match manual_id {
            Some(manual) => (manual, true),
            None => {
                // Auto-slugs only for levels within the cap (0 = disabled).
                if self.heading_ids.max_level == 0 || heading.level > self.heading_ids.max_level {
                    return String::new();
                }
                let slug = heading_auto_slug(node);
                if slug.is_empty() {
                    return String::new();
                }
                (slug, false)
            }
        };

        if self.seen_ids.contains(&id) {
            let warning = if is_manual {
                format!(
                    "duplicate heading ID \"{}\" — already used by an earlier heading; the later {{#{}}} is ignored",
                    id, id
                )
            } else {
                format!(
                    "duplicate heading ID \"{}\" — use a manual ID like `# Heading {{#custom-id}}` or set headingIds.maxLevel to limit auto-generation",
                    id
                )
            };
            self.warnings.push(warning);
            return String::new(); // Don't add the ID for duplicates.
        }

        // The ID attaches to the heading as a Typst label. The `label()`
        // function (not the `<id>` literal) takes a string argument, so
        // manual IDs with spaces/quotes/unicode stay valid once escaped; the
        // call renders nothing. Format from the borrow, then move the ID
        // into the seen set — no clone.
        let suffix = format!(" #label(\"{}\")", escape_typst_string(&id));
        self.seen_ids.insert(id);
        suffix
    }

    /// Render a node's inline children into escaped Typst markup.
    fn render_inlines(&self, node: Node) -> String {
        let mut writer = Writer::new();
        self.render_inlines_into(node, &mut writer);
        writer.finish()
    }

    /// Accumulator form of `render_inlines`: appends to `writer` instead of
    /// returning a string, so container nodes wrap their children without
    /// per-node intermediate strings.
    fn render_inlines_into(&self, node: Node, writer: &mut Writer) {
        for child in node.children() {
            self.render_inline_into(child, writer);
        }
    }

    /// Append one inline node's subtree to `writer`.
    ///
    /// Iterative: comrak caps list nesting but nothing bounds INLINE nesting
    /// (it grows with line length — `***…***` on one line), so a recursive
    /// walk could overflow the stack. `traverse()` yields Start/End events
    /// in tree order, which map 1:1 onto the accumulator's prefix/suffix
    /// writes. `depth`/`skip_below` preserve the recursive version's
    /// drop-unknown-subtree behavior: a node matched by no arm removes its
    /// whole subtree from the output.
    fn render_inline_into(&self, node: Node, writer: &mut Writer) {
        let mut depth = 0usize;
        let mut skip_below: Option<usize> = None;
        for edge in node.traverse() {
            match edge {
                NodeEdge::Start(n) => {
                    if skip_below.is_some() {
                        depth += 1;
                        continue;
                    }
                    match &n.data().value {
                        NodeValue::Text(text) => escape_text_into(text, &mut writer.buf),
                        NodeValue::SoftBreak => writer.push_str(" "),
                        // `\ ` (backslash + space) is a hard line break in Typst markup.
                        NodeValue::LineBreak => writer.push_str("\\ "),
                        NodeValue::Code(code) => {
                            // The prelude's `inline-code` owns the styling; the string
                            // argument avoids the backtick-fence branch entirely (§6.2).
                            writer.push_str("#inline-code(\"");
                            escape_typst_string_into(&code.literal, &mut writer.buf);
                            writer.push_str("\")");
                        }
                        NodeValue::Strong => writer.push_str("*"),
                        NodeValue::Emph => writer.push_str("_"),
                        NodeValue::Strikethrough => writer.push_str("#strike["),
                        // `==mark==`, `^sup^`, `~sub~` (Custom flavor, §4) —
                        // the prelude helpers own the styling.
                        NodeValue::Highlight => writer.push_str("#mark["),
                        NodeValue::Superscript => writer.push_str("#superscript["),
                        NodeValue::Subscript => writer.push_str("#subscript["),
                        NodeValue::Link(link) => {
                            // A `#fragment` URL targets a heading label (§6.2). The prelude's
                            // `internal-link` owns the guard: a missing label degrades to
                            // plain text instead of failing the compile. An empty fragment
                            // stays a URL link.
                            if let Some(fragment) =
                                link.url.strip_prefix('#').filter(|f| !f.is_empty())
                            {
                                writer.push_str("#internal-link(\"");
                                escape_typst_string_into(fragment, &mut writer.buf);
                            } else {
                                writer.push_str("#link(\"");
                                escape_typst_string_into(&link.url, &mut writer.buf);
                            }
                            writer.push_str("\")[");
                        }
                        NodeValue::Image(link) => {
                            // The prelude's `figure-image` owns the default width and caption
                            // placement; Rust passes the URL (data) and the alt text (content).
                            writer.push_str("#figure-image(\"");
                            escape_typst_string_into(&link.url, &mut writer.buf);
                            writer.push_str("\", [");
                        }
                        NodeValue::HtmlInline(html) => escape_text_into(html, &mut writer.buf),
                        // Passthrough: LaTeX macros are not converted in v1 (§6.2).
                        NodeValue::Math(math) => {
                            if math.display_math {
                                writer.push_str("$ ");
                                writer.push_str(&math.literal);
                                writer.push_str(" $");
                            } else {
                                writer.push_str("$");
                                writer.push_str(&math.literal);
                                writer.push_str("$");
                            }
                        }
                        // A footnote reference is inlined at the call site — `#footnote[...]`
                        // attaches the superscript marker where it is called (§6.2).
                        NodeValue::FootnoteReference(reference) => {
                            if let Some(content) = self.footnotes.get(&reference.name) {
                                writer.push_str("#footnote[");
                                writer.push_str(content);
                                writer.push_str("]");
                            }
                            // Dangling reference: drop rather than emit a broken marker.
                        }
                        // Unknown node: drop the whole subtree.
                        _ => skip_below = Some(depth),
                    }
                    depth += 1;
                }
                NodeEdge::End(n) => {
                    depth -= 1;
                    if let Some(skip) = skip_below {
                        if depth == skip {
                            skip_below = None;
                        }
                        continue;
                    }
                    match &n.data().value {
                        NodeValue::Strong => writer.push_str("*"),
                        NodeValue::Emph => writer.push_str("_"),
                        NodeValue::Strikethrough
                        | NodeValue::Highlight
                        | NodeValue::Superscript
                        | NodeValue::Subscript
                        | NodeValue::Link(_) => writer.push_str("]"),
                        NodeValue::Image(_) => writer.push_str("])"),
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Append-only markup accumulator: renderers write fragments in emission
/// order instead of returning per-node strings bottom-up, which avoids an
/// intermediate allocation (and re-copy) at every nesting level. Block
/// renderers still return strings and get one `Writer` per inline span.
struct Writer {
    buf: String,
}

impl Writer {
    fn new() -> Self {
        Self { buf: String::new() }
    }

    fn push_str(&mut self, fragment: &str) {
        self.buf.push_str(fragment);
    }

    fn finish(self) -> String {
        self.buf
    }
}

/// The heading's auto-slug: the kebab-case of its inline plain text —
/// lowercased, non-alphanumeric runs collapsed to a single hyphen, ends
/// trimmed. The slug is folded directly from the AST text, one chunk at a
/// time, with no intermediate text buffer.
///
/// Iterative like `render_inline_into`: inline nesting depth grows with
/// line length, so a recursive walk could overflow the stack.
fn heading_auto_slug(node: Node) -> String {
    let mut slug = String::new();
    let mut depth = 0usize;
    let mut skip_below: Option<usize> = None;
    for edge in node.traverse() {
        match edge {
            NodeEdge::Start(n) => {
                if skip_below.is_some() {
                    depth += 1;
                    continue;
                }
                match &n.data().value {
                    NodeValue::Text(t) => {
                        // Strip heading attribute syntax like `{#id}`; fold
                        // the remaining pieces as literal text.
                        let mut rest = t.as_ref();
                        while let Some(start) = rest.find("{#") {
                            fold_slug(&mut slug, &rest[..start]);
                            // Skip past the attribute: `end` is the offset of
                            // `}` within `rest[start..]`, so the text resumes
                            // at `start + end + 1`.
                            let Some(end) = rest[start..].find('}') else {
                                // No closing brace — literal text to the end.
                                fold_slug(&mut slug, &rest[start..]);
                                rest = "";
                                break;
                            };
                            rest = &rest[start + end + 1..];
                        }
                        fold_slug(&mut slug, rest);
                    }
                    NodeValue::Code(code) => fold_slug(&mut slug, &code.literal),
                    NodeValue::HtmlInline(html) => fold_slug(&mut slug, html),
                    // Setext headings span multiple lines: breaks render as
                    // whitespace in the heading body, so they separate words
                    // in the slug too.
                    NodeValue::SoftBreak | NodeValue::LineBreak => fold_slug(&mut slug, " "),
                    // Strikethrough content is excluded from slugs; math is a
                    // leaf and contributes nothing.
                    NodeValue::Strikethrough => skip_below = Some(depth),
                    _ => {}
                }
                depth += 1;
            }
            NodeEdge::End(_) => {
                depth -= 1;
                if let Some(skip) = skip_below {
                    if depth == skip {
                        skip_below = None;
                    }
                }
            }
        }
    }
    // Hyphens collapse as they are pushed, so at most one trailing hyphen
    // can exist — drop it (leading ones are never pushed).
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Compute the heading's slug (manual ID or auto-slug) without side effects.
/// Used by `collect_headings` to populate TOC items — the block renderer
/// does the full uniqueness check and label suffix generation.
fn heading_slug(node: Node, heading_ids: &HeadingIds) -> String {
    let NodeValue::Heading(heading) = &node.data().value else {
        return String::new();
    };
    // Manual `{#custom-id}` wins over the auto slug and bypasses the cap.
    if let Some(manual) = node.data().attrs.as_ref().and_then(|a| a.id.clone()) {
        return manual;
    }
    // Auto-slugs only for levels within the cap (0 = disabled).
    if heading_ids.max_level == 0 || heading.level > heading_ids.max_level {
        return String::new();
    }
    heading_auto_slug(node)
}

/// Fold one chunk of heading text into the growing slug: lowercase, keep
/// alphanumerics, collapse every non-alphanumeric run to a single hyphen.
/// Leading non-alphanumerics are dropped (the slug never starts with a
/// hyphen); chunk boundaries are irrelevant to the fold.
fn fold_slug(slug: &mut String, chunk: &str) {
    for c in chunk.to_lowercase().chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
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

#[cfg(test)]
mod tests {
    use super::md_to_typst;
    use crate::options::{Flavor, HeadingIds};

    /// Default heading ID config: enabled for all 6 levels.
    fn default_heading_ids() -> HeadingIds {
        HeadingIds { max_level: 6 }
    }

    #[test]
    fn empty_input_returns_empty_body() {
        for input in ["", "   ", "\n\n"] {
            let doc = md_to_typst(input, Flavor::Github, &default_heading_ids());
            assert_eq!(doc.body, "[]", "input: {input:?}");
            assert_eq!(doc.title, None);
            assert_eq!(doc.author, None);
        }
    }

    #[test]
    fn maps_headings_to_typst_levels() {
        let doc = md_to_typst(
            "# Title\n\n## Sub\n\n### Third",
            Flavor::Github,
            &default_heading_ids(),
        );
        // IDs attach as Typst labels via the `label()` function — the
        // string argument accepts any ID (escaped), and the call renders
        // nothing, unlike the old `{\#id}` text.
        assert_eq!(
            doc.body,
            "= Title #label(\"title\")\n\n== Sub #label(\"sub\")\n\n=== Third #label(\"third\")"
        );
    }

    /// Auto-slug: generates a kebab-case ID from the heading text.
    #[test]
    fn auto_slug_heading_id_kebab_case() {
        let doc = md_to_typst(
            "# Hello World\n\n## Foo Bar Baz",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            doc.body,
            "= Hello World #label(\"hello-world\")\n\n== Foo Bar Baz #label(\"foo-bar-baz\")"
        );
    }

    /// Auto-slug: special characters become hyphens, consecutive runs collapse.
    #[test]
    fn auto_slug_heading_id_special_chars() {
        let doc = md_to_typst("# C++ & Rust!", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "= C++ & Rust! #label(\"c-rust\")");
    }

    /// Manual ID: `{#custom-id}` overrides the auto-generated slug.
    #[test]
    fn manual_heading_id_overrides_auto_slug() {
        let doc = md_to_typst(
            "# My Title {#my-title}",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "= My Title #label(\"my-title\")");
    }

    /// Duplicate auto-slugs: first wins, subsequent duplicates get a warning.
    #[test]
    fn duplicate_heading_ids_emit_warning() {
        let doc = md_to_typst(
            "# Section\n\n## Section",
            Flavor::Github,
            &default_heading_ids(),
        );
        // First heading gets the ID, second one (duplicate) doesn't.
        assert_eq!(doc.body, "= Section #label(\"section\")\n\n== Section");
        assert!(
            doc.warnings.iter().any(|w| w.contains("duplicate")),
            "expected duplicate warning: {:?}",
            doc.warnings
        );
    }

    /// `max_level: 0` disables only AUTO-slug generation. A manual `{#id}`
    /// is the author's explicit intent and stays honored at every level;
    /// headings without one simply get no ID.
    #[test]
    fn manual_heading_ids_survive_max_level_zero() {
        let doc = md_to_typst(
            "# Title {#t}\n\n## Sub {#s}\n\n### Deep",
            Flavor::Github,
            &HeadingIds { max_level: 0 },
        );
        assert_eq!(
            doc.body,
            "= Title #label(\"t\")\n\n== Sub #label(\"s\")\n\n=== Deep"
        );
        assert!(doc.warnings.is_empty());
    }

    /// The level cap limits AUTO-slug generation only: manual IDs are
    /// honored above the cap too, while deeper headings without a manual
    /// ID get no slug.
    #[test]
    fn heading_level_cap_limits_auto_slugs_only() {
        let doc = md_to_typst(
            "# H1\n\n## H2\n\n### H3 {#h3}\n\n### H4",
            Flavor::Github,
            &HeadingIds { max_level: 2 },
        );
        assert_eq!(
            doc.body,
            "= H1 #label(\"h1\")\n\n== H2 #label(\"h2\")\n\n=== H3 #label(\"h3\")\n\n=== H4"
        );
        assert!(doc.warnings.is_empty());
    }

    /// Duplicate manual IDs collide like duplicate auto-slugs: Typst accepts
    /// duplicate labels at compile time but any reference to them fails, so
    /// the first occurrence wins and the duplicate is dropped with a warning.
    #[test]
    fn duplicate_manual_heading_ids_emit_warning() {
        let doc = md_to_typst(
            "# A {#x}\n\n## B {#x}",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "= A #label(\"x\")\n\n== B");
        assert!(
            doc.warnings.iter().any(|w| w.contains("duplicate")),
            "expected duplicate warning: {:?}",
            doc.warnings
        );
    }

    /// Manual IDs and auto-slugs share one uniqueness namespace in both
    /// directions: whichever comes first wins, the later collision is
    /// dropped with a warning.
    #[test]
    fn manual_and_auto_ids_share_one_uniqueness_namespace() {
        // A manual ID colliding with an earlier auto-slug.
        let manual_late = md_to_typst(
            "# Section\n\n## Other {#section}",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            manual_late.body,
            "= Section #label(\"section\")\n\n== Other"
        );
        assert!(manual_late.warnings.iter().any(|w| w.contains("duplicate")));

        // An auto-slug colliding with an earlier manual ID.
        let auto_late = md_to_typst(
            "# Other {#section}\n\n## Section",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(auto_late.body, "= Other #label(\"section\")\n\n== Section");
        assert!(auto_late.warnings.iter().any(|w| w.contains("duplicate")));
    }

    /// `{#…` that comrak leaves as text (attribute syntax only parses at
    /// end of line) is stripped from the slug WITHOUT eating the characters
    /// after it — the text still renders literally in the heading.
    #[test]
    fn mid_heading_attribute_syntax_keeps_following_slug_text() {
        let doc = md_to_typst("# a {#x} b", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "= a {\\#x} b #label(\"a-b\")");
    }

    /// An unclosed `{#…` is literal text: it contributes its characters to
    /// the slug exactly once (the old walk emitted the tail twice).
    #[test]
    fn unclosed_heading_attribute_syntax_is_literal_slug_text() {
        let doc = md_to_typst("# a {#x", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "= a {\\#x #label(\"a-x\")");

        let doc = md_to_typst("# a {#x b", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "= a {\\#x b #label(\"a-x-b\")");
    }

    /// Setext headings span multiple lines, so their inline content can
    /// contain soft breaks (and hard breaks via trailing spaces). Both
    /// render as whitespace in the heading body, so they must separate
    /// words in the slug rather than gluing them together.
    #[test]
    fn setext_heading_breaks_separate_slug_words() {
        let soft = md_to_typst(
            "First line\nsecond line\n===========",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            soft.body,
            "= First line second line #label(\"first-line-second-line\")"
        );

        let hard = md_to_typst(
            "First  \nsecond\n======",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(hard.body, "= First\\ second #label(\"first-second\")");
    }

    #[test]
    fn maps_emphasis_strong_and_strikethrough() {
        let doc = md_to_typst(
            "**bold** and *em* and ~~strike~~",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "*bold* and _em_ and #strike[strike]");
    }

    /// Nested inline containers interleave their wrappers in tree order —
    /// the safety net for the Start/End event traversal (depth ≥ 2).
    #[test]
    fn nested_inline_containers_interleave_wrappers() {
        let doc = md_to_typst(
            "**bold *em* and [link](https://x) end** plus [*inner*](https://y) tail",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            doc.body,
            "*bold _em_ and #link(\"https://x\")[link] end* plus #link(\"https://y\")[_inner_] tail"
        );
    }

    #[test]
    fn soft_breaks_become_spaces_hard_breaks_escaped() {
        let soft = md_to_typst("line one\nline two", Flavor::Github, &default_heading_ids());
        assert_eq!(soft.body, "line one line two");

        let hard = md_to_typst(
            "line one\\\nline two",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(hard.body, "line one\\ line two");
    }

    #[test]
    fn maps_inline_code_and_backtick_containing_code() {
        let plain = md_to_typst("use `code` here", Flavor::Github, &default_heading_ids());
        assert_eq!(plain.body, "use #inline-code(\"code\") here");

        // ``a `b``  →  a code span containing a backtick; the string argument
        // needs no fence-length math.
        let backtick = md_to_typst("``a `b``", Flavor::Github, &default_heading_ids());
        assert_eq!(backtick.body, "#inline-code(\"a `b\")");

        // A literal with a backtick *and* quotes/backslashes → escaped string.
        let quoted = md_to_typst("```a `b \"c\\```", Flavor::Github, &default_heading_ids());
        assert_eq!(quoted.body, "#inline-code(\"a `b \\\"c\\\\\")");
    }

    #[test]
    fn maps_code_blocks() {
        let rust = md_to_typst(
            "```rust\nfn main() {}\n```",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(rust.body, "#code-block(\"rust\", \"fn main() {}\")");

        // No language → the empty string; the prelude owns the fallback.
        let no_lang = md_to_typst("```\nplain\n```", Flavor::Github, &default_heading_ids());
        assert_eq!(no_lang.body, "#code-block(\"\", \"plain\")");

        // A literal containing a backtick run needs no longer fence.
        let backticks = md_to_typst("```\na ``` b\n```", Flavor::Github, &default_heading_ids());
        assert_eq!(backticks.body, "#code-block(\"\", \"a ``` b\")");
    }

    #[test]
    fn maps_links_and_images() {
        let doc = md_to_typst(
            "[label](https://example.com) and ![alt](img.png)",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            doc.body,
            "#link(\"https://example.com\")[label] and #figure-image(\"img.png\", [alt])"
        );

        // A quote in the URL is escaped inside the Typst string literal.
        let quoted = md_to_typst(
            "[x](<https://a.com/\"q\">)",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(quoted.body, "#link(\"https://a.com/\\\"q\\\"\")[x]");
    }

    /// A `#fragment` URL targets a heading label (§6.2): the converter
    /// emits the prelude's `internal-link` helper, which guards against
    /// missing labels (a broken TOC entry degrades to plain text instead
    /// of failing the render). External URLs keep the plain string form;
    /// an empty fragment (`#`) stays a URL link.
    #[test]
    fn maps_internal_links_to_label_links() {
        let internal = md_to_typst(
            "[Section](#section-id)",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(internal.body, "#internal-link(\"section-id\")[Section]");

        let external = md_to_typst(
            "[home](https://example.com)",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(external.body, "#link(\"https://example.com\")[home]");

        let empty = md_to_typst("[top](#)", Flavor::Github, &default_heading_ids());
        assert_eq!(empty.body, "#link(\"#\")[top]");
    }

    #[test]
    fn maps_bullet_and_ordered_lists() {
        // Bullet lists stay native Typst `- ` lists (themeable via #show list).
        let bullet = md_to_typst("- a\n- b", Flavor::Github, &default_heading_ids());
        assert_eq!(bullet.body, "- a\n- b");

        // Ordered lists use the prelude helper (a native enum) so the
        // numbering is themeable; the markdown start value is preserved.
        let ordered = md_to_typst("1. a\n2. b", Flavor::Github, &default_heading_ids());
        assert_eq!(ordered.body, "#ordered-list(1, [a], [b])");

        let starts_at_three = md_to_typst("3. a\n4. b", Flavor::Github, &default_heading_ids());
        assert_eq!(starts_at_three.body, "#ordered-list(3, [a], [b])");
    }

    #[test]
    fn maps_nested_lists_with_indentation() {
        let nested = md_to_typst("- a\n  - b", Flavor::Github, &default_heading_ids());
        assert_eq!(nested.body, "- a\n  - b");
    }

    #[test]
    fn maps_task_list_items() {
        // Task lists become calls to the prelude helpers, each owning the
        // checkbox glyph and content cell (§6.2).
        let doc = md_to_typst(
            "- [x] done\n- [ ] todo",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "#task-list(task-done([done]), task-todo([todo]))");
    }

    #[test]
    fn maps_plain_bullet_item_mixed_into_a_task_list() {
        // GFM allows a non-checkbox bullet item in the same list; it gets the
        // `task-plain` helper (default bullet glyph in the checkbox column).
        let doc = md_to_typst(
            "- [x] done\n- plain item",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            doc.body,
            "#task-list(task-done([done]), task-plain([plain item]))"
        );
    }

    #[test]
    fn loose_list_items_keep_paragraph_breaks() {
        let doc = md_to_typst("- a\n\n  continued", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "- a\n\n  continued");
    }

    #[test]
    fn maps_gfm_tables_only_in_gfm_and_custom_flavors() {
        let gfm = md_to_typst(
            "| a | b |\n|---|---|\n| 1 | 2 |",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            gfm.body,
            "#markdown-table(columns: 2, header: ([a], [b]), rows: (([1], [2]),))"
        );

        // CommonMark has no table extension: the same source is paragraphs.
        let commonmark = md_to_typst(
            "| a | b |\n|---|---|\n| 1 | 2 |",
            Flavor::Commonmark,
            &default_heading_ids(),
        );
        assert!(commonmark.body.contains("| a | b |"), "{}", commonmark.body);
    }

    #[test]
    fn maps_blockquotes_and_thematic_breaks() {
        let quote = md_to_typst("> quoted text", Flavor::Github, &default_heading_ids());
        assert_eq!(quote.body, "#blockquote[quoted text]");

        let hr = md_to_typst("a\n\n---\n\nb", Flavor::Github, &default_heading_ids());
        assert_eq!(hr.body, "a\n\n#thematic-break()\n\nb");
    }

    #[test]
    fn strips_html_to_escaped_plain_text() {
        // Inside an HTML *block*, content is not parsed as markdown, so the
        // whole literal is emitted as escaped plain text.
        let block = md_to_typst(
            "<div>\n<p>*hi*</p>\n</div>",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(block.body, "<div>\n<p>\\*hi\\*</p>\n</div>");

        // Inline HTML tags pass through; the surrounding markdown is parsed.
        let inline = md_to_typst(
            "a <span>*x*</span> b",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(inline.body, "a <span>_x_</span> b");
    }

    /// The pandoc-convention `<!-- pagebreak -->` comment forces a page
    /// break when it is a whole block (Custom flavor, §4). Whitespace
    /// inside the comment is tolerated; the marker itself is case-sensitive.
    #[test]
    fn page_break_marker_becomes_pagebreak_in_custom_flavor() {
        let doc = md_to_typst(
            "before\n\n<!-- pagebreak -->\n\nafter",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "before\n\n#pagebreak()\n\nafter");

        let tight = md_to_typst("<!--pagebreak-->", Flavor::Custom, &default_heading_ids());
        assert_eq!(tight.body, "#pagebreak()");

        let indented = md_to_typst(
            "  <!-- pagebreak -->",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(indented.body, "#pagebreak()");
    }

    /// Page breaks are a Custom-flavor extension (§4): in CommonMark/GFM
    /// the marker stays lossy text like any other HTML.
    #[test]
    fn page_break_marker_is_custom_flavor_only() {
        for flavor in [Flavor::Commonmark, Flavor::Github] {
            let doc = md_to_typst(
                "a\n\n<!-- pagebreak -->\n\nb",
                flavor,
                &default_heading_ids(),
            );
            assert!(!doc.body.contains("#pagebreak()"), "flavor: {flavor:?}");
            assert!(doc.body.contains("pagebreak"), "flavor: {flavor:?}");
        }
    }

    /// Only a whole-block marker is a page break: other comments stay lossy
    /// text, and an inline marker inside a paragraph is not block-level.
    #[test]
    fn page_break_marker_requires_a_whole_block() {
        let other = md_to_typst(
            "<!-- not a break -->",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert!(!other.body.contains("#pagebreak()"));

        let inline = md_to_typst(
            "text <!-- pagebreak --> more",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert!(!inline.body.contains("#pagebreak()"));
    }

    #[test]
    fn maps_footnotes_and_math_in_custom_flavor_only() {
        // A footnote reference is inlined at its call site — `#footnote[...]`
        // puts the superscript marker where it is called (§6.2); the
        // definition block itself is dropped from the flow.
        let custom = md_to_typst(
            "text[^1]\n\n[^1]: the note",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(custom.body, "text#footnote[the note]");

        let math = md_to_typst(
            "Inline $x^2$ and display $$y$$.",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(math.body, "Inline $x^2$ and display $ y $.");

        // Without the extension the syntax is ordinary (escaped) text.
        let gfm = md_to_typst("text[^1] and $x^2$", Flavor::Github, &default_heading_ids());
        assert_eq!(gfm.body, "text\\[^1\\] and \\$x^2\\$");
    }

    #[test]
    fn maps_highlight_superscript_subscript_in_custom_flavor_only() {
        let highlight = md_to_typst("a ==marked== b", Flavor::Custom, &default_heading_ids());
        assert_eq!(highlight.body, "a #mark[marked] b");

        let superscript = md_to_typst("x^sup^", Flavor::Custom, &default_heading_ids());
        assert_eq!(superscript.body, "x#superscript[sup]");

        let subscript = md_to_typst("H~sub~O", Flavor::Custom, &default_heading_ids());
        assert_eq!(subscript.body, "H#subscript[sub]O");

        // Nesting composes with the other inline wrappers.
        let nested = md_to_typst("**==both==**", Flavor::Custom, &default_heading_ids());
        assert_eq!(nested.body, "*#mark[both]*");

        // Without the extensions the markers stay literal text.
        let github = md_to_typst("a ==marked== b", Flavor::Github, &default_heading_ids());
        assert_eq!(github.body, "a ==marked== b");
    }

    #[test]
    fn maps_definition_lists_in_custom_flavor_only() {
        let single = md_to_typst(
            "Term 1\n\n: Details 1",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(single.body, "#definition-list(([Term 1], [Details 1]))");

        let multi = md_to_typst(
            "Term 1\n\n: Details 1\n\nTerm 2\n\n: Details 2",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(
            multi.body,
            "#definition-list(([Term 1], [Details 1]), ([Term 2], [Details 2]))"
        );
    }

    /// Regression guard: every Custom-flavor construct actually renders.
    /// Before these render arms existed, comrak parsed the constructs but the
    /// converter dropped them, so selecting Custom silently lost content.
    #[test]
    fn custom_flavor_renders_every_construct() {
        let md = "==marked==\n\nx^sup^ and H~sub~O\n\nTerm\n\n: Details";
        let doc = md_to_typst(md, Flavor::Custom, &default_heading_ids());
        assert!(doc.body.contains("#mark[marked]"), "{}", doc.body);
        assert!(doc.body.contains("#superscript[sup]"), "{}", doc.body);
        assert!(doc.body.contains("#subscript[sub]"), "{}", doc.body);
        assert!(
            doc.body.contains("#definition-list(([Term], [Details]))"),
            "{}",
            doc.body
        );
    }

    #[test]
    fn maps_footnote_reference_inside_a_list_item() {
        // The marker must stay inline in the bullet item, not drift to the
        // definition at the end of the document.
        let custom = md_to_typst(
            "- bullet [^1]\n- another item\n\n[^1]: the note",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(custom.body, "- bullet #footnote[the note]\n- another item");
    }

    #[test]
    fn leaves_unresolved_footnote_syntax_as_literal_text() {
        // comrak only recognizes `[^1]` as a footnote reference when a
        // matching definition exists; otherwise it is ordinary escaped text.
        let custom = md_to_typst("text[^1]", Flavor::Custom, &default_heading_ids());
        assert_eq!(custom.body, "text\\[^1\\]");
    }

    #[test]
    fn parses_front_matter_title_and_author() {
        let doc = md_to_typst(
            "---\ntitle: \"My Doc\"\nauthor: \"Ada\"\n---\n\n# Body",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(doc.title.as_deref(), Some("My Doc"));
        assert_eq!(doc.author.as_deref(), Some("Ada"));
        assert_eq!(doc.body, "= Body #label(\"body\")");
    }

    #[test]
    fn front_matter_is_optional_and_unquoted_values_work() {
        let unquoted = md_to_typst(
            "---\ntitle: Plain Doc\n---\n\nBody",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(unquoted.title.as_deref(), Some("Plain Doc"));
        assert_eq!(unquoted.author, None);

        let none = md_to_typst("# No front matter", Flavor::Custom, &default_heading_ids());
        assert_eq!(none.title, None);
        assert_eq!(none.author, None);
    }

    #[test]
    fn maps_indented_code_blocks() {
        let doc = md_to_typst("    let x = 1;\n", Flavor::Github, &default_heading_ids());
        assert_eq!(doc.body, "#code-block(\"\", \"let x = 1;\")");

        let blank = md_to_typst("    a\n\n    b\n", Flavor::Github, &default_heading_ids());
        assert_eq!(blank.body, "#code-block(\"\", \"a\n\nb\")");
    }

    #[test]
    fn maps_autolinks_without_label() {
        // GFM autolink: the URL is also the link text.
        let doc = md_to_typst(
            "see <https://example.com> now",
            Flavor::Github,
            &default_heading_ids(),
        );
        assert_eq!(
            doc.body,
            "see #link(\"https://example.com\")[https://example.com] now"
        );
    }

    /// Document-level override: the front-matter `autoHeadingLevel` caps
    /// AUTO-slug generation for this document, beating the toolbar setting
    /// (here: all 6 levels). Manual IDs are unaffected (§6.2).
    #[test]
    fn front_matter_auto_heading_level_overrides_option() {
        let doc = md_to_typst(
            "---\nautoHeadingLevel: 1\n---\n\n# One\n\n## Two\n\n### Three {#tres}",
            Flavor::Custom,
            &default_heading_ids(),
        );
        // Level 1 auto-slug kept, level 2 auto-slug suppressed by the cap,
        // level 3 manual ID honored despite the cap.
        assert_eq!(
            doc.body,
            "= One #label(\"one\")\n\n== Two\n\n=== Three #label(\"tres\")"
        );
        assert!(doc.warnings.is_empty());
        assert_eq!(doc.auto_heading_level, 1);
    }

    /// An invalid `autoHeadingLevel` warns and keeps the toolbar setting
    /// rather than failing the render (§6.2).
    #[test]
    fn front_matter_invalid_auto_heading_level_warns_and_keeps_option() {
        let doc = md_to_typst(
            "---\nautoHeadingLevel: abc\n---\n\n# One\n\n## Two",
            Flavor::Custom,
            &default_heading_ids(),
        );
        // Toolbar default (all levels) still applies.
        assert_eq!(doc.body, "= One #label(\"one\")\n\n== Two #label(\"two\")");
        assert_eq!(doc.auto_heading_level, 6);
        assert!(
            doc.warnings
                .iter()
                .any(|w| w.contains("autoHeadingLevel") && w.contains("abc")),
            "expected an invalid-value warning: {:?}",
            doc.warnings
        );
    }

    /// `autoHeadingLevel: 0` in front matter disables auto-slugs for the
    /// document while manual IDs stay honored — same semantics as the
    /// toolbar setting (§6.2).
    #[test]
    fn front_matter_auto_heading_level_zero_keeps_manual_ids() {
        let doc = md_to_typst(
            "---\nautoHeadingLevel: 0\n---\n\n# One {#uno}\n\n## Two",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(doc.body, "= One #label(\"uno\")\n\n== Two");
        assert!(doc.warnings.is_empty());
        assert_eq!(doc.auto_heading_level, 0);
    }

    /// Unknown front-matter keys are tolerated (title/author still parse)
    /// but produce a warning — with a suggestion when the key is a likely
    /// typo (§6.2).
    #[test]
    fn front_matter_unknown_keys_warn_but_still_parse() {
        let doc = md_to_typst(
            "---\ntitle: \"T\"\ndate: \"2026-01-01\"\ntags: [a]\nauthor: \"A\"\n---\n\nBody",
            Flavor::Custom,
            &default_heading_ids(),
        );
        assert_eq!(doc.title.as_deref(), Some("T"));
        assert_eq!(doc.author.as_deref(), Some("A"));
        assert_eq!(doc.warnings.len(), 2, "{:?}", doc.warnings);
        assert!(doc.warnings.iter().any(|w| w.contains("date")));
        assert!(doc.warnings.iter().any(|w| w.contains("tags")));
    }

    /// Front matter is parsed in every flavor, not just Custom (§6.2).
    #[test]
    fn front_matter_works_in_every_flavor() {
        for flavor in [Flavor::Commonmark, Flavor::Github, Flavor::Custom] {
            let doc = md_to_typst("---\ntitle: T\n---\n\nBody", flavor, &default_heading_ids());
            assert_eq!(doc.title.as_deref(), Some("T"), "flavor: {flavor:?}");
        }
    }

    /// A document-declared flavor overrides the toolbar flavor (§6.2): this
    /// document forces CommonMark, so the GFM table stays literal text.
    #[test]
    fn front_matter_flavor_overrides_toolbar() {
        let md = "---\nflavor: commonmark\n---\n\n| a | b |\n|---|---|\n| 1 | 2 |";
        let doc = md_to_typst(md, Flavor::Github, &default_heading_ids());
        assert_eq!(doc.flavor, Flavor::Commonmark);
        assert!(doc.body.contains("| a | b |"), "{}", doc.body);
    }

    /// `[[_TOC_]]` is collected as TocItems and emitted as a `#markdown-toc(...)` call.
    #[test]
    fn toc_collected_and_emitted() {
        let doc = md_to_typst(
            "---\nflavor: custom\n---\n\n# First\n\n## Second\n\n[[_TOC_]]",
            Flavor::Custom,
            &HeadingIds { max_level: 6 },
        );
        // Headings collected for TOC.
        assert_eq!(doc.toc.len(), 2);
        assert_eq!(doc.toc[0].level, 1);
        assert_eq!(doc.toc[0].text, "First");
        assert_eq!(doc.toc[0].slug, "first");
        assert_eq!(doc.toc[1].level, 2);
        assert_eq!(doc.toc[1].text, "Second");
        assert_eq!(doc.toc[1].slug, "second");
        // HtmlBlock `[[_TOC_]]` → #markdown-toc(...)
        assert!(doc.body.contains("#markdown-toc(("));
        assert!(doc.body.contains("1, [First], \"first\""));
        assert!(doc.body.contains("2, [Second], \"second\""));
    }

    /// `[[_TOC_]]` with no headings emits an empty list.
    #[test]
    fn toc_empty_list_when_no_headings() {
        let doc = md_to_typst(
            "---\nflavor: custom\n---\n\nSome text.\n\n[[_TOC_]]",
            Flavor::Custom,
            &HeadingIds { max_level: 6 },
        );
        assert_eq!(doc.toc.len(), 0);
        assert!(doc.body.contains("#markdown-toc(("));
    }

    /// Headings without IDs are excluded from TOC items when auto-slugs
    /// are disabled (max_level: 0).
    #[test]
    fn toc_excludes_headings_without_ids_when_no_auto_slug() {
        let doc = md_to_typst(
            "---\nflavor: custom\n---\n\n# Has ID {#hid}\n\n## No ID\n\n[[_TOC_]]",
            Flavor::Custom,
            &HeadingIds { max_level: 0 },
        );
        assert_eq!(doc.toc.len(), 2);
        // Only heading with explicit ID should be in the emitted TOC call.
        assert!(doc.body.contains("1, [Has ID], \"hid\""));
        // The TOC call should not contain "No ID" — it's excluded because
        // it has no slug (empty slug filtered out).
        let toc_call_start = doc.body.find("#markdown-toc(").unwrap();
        let toc_call_end = doc.body[toc_call_start..].find(")").unwrap() + toc_call_start;
        let toc_call = &doc.body[toc_call_start..=toc_call_end];
        assert!(!toc_call.contains("No ID"), "TOC call: {toc_call}");
    }
}
