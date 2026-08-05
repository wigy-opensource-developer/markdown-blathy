// default.typ — embedded default template for tauri-md-editor.
// Contract: must define #let article(...) with these exact parameter names.
// Users may override this file; a custom template is validated by compiling
// a probe document before being accepted (§7.4).

#set page(paper: "a4", margin: (x: 20mm, y: 25mm))
#set text(font: "DejaVu Sans", size: 11pt)
#set par(justify: false)
// Headings: weight is a text property in Typst 0.15+ (`heading` no longer
// takes `weight` directly).
#show heading: set text(weight: 600)

// Code blocks: light background, monospace, no highlighting in v1.
#show raw.where(block: true): it => block(
  fill: rgb("#f5f5f5"), inset: 8pt, radius: 4pt, it,
)

// Links: styled like the classic markdown look.
#show link: set text(fill: rgb("#0366d6"))

// Task-list helper used by the converter (§6.2): a two-column grid (checkbox
// glyph + content) so checked and unchecked items can mix at one level — a
// native Typst list has a single marker for all items. The converter emits
// alternating cells: `#task-list([#sym.ballot] [todo], [#sym.ballot.check]
// [done],)`.
//
// `row-gutter` matches the default `par.leading` (0.65em), so the space
// between rows equals the space between wrapped lines inside a row — mixed
// single-line and multi-line tasks keep one uniform line rhythm.
#let task-list(..cells) = grid(
  columns: (auto, 1fr),
  column-gutter: 8pt,
  row-gutter: 0.65em,
  ..cells,
)

// Blockquote helper used by the converter (§6.2). Typst's `quote` element is
// native since 0.13; `block: true` renders it as a standalone block quote
// (left border + spacing via the standard library's show rule).
#let blockquote(body) = quote(block: true)[#body]

// Optional title/author from YAML front matter (§6.2), rendered as a
// centered title block when present.
#let title-block(title, author) = {
  set align(center)
  text(size: 1.6em, weight: 700)[#title]
  if author != none {
    linebreak()
    text(size: 1.1em, weight: 500)[#author]
  }
  v(0.8em)
}

// Main entry point. The Rust side always appends: #article(...)[body]
#let article(
  paper: "a4",
  margin-x: 20mm,
  margin-y: 25mm,
  text-align: "left",
  title: none,
  author: none,
  body,
) = {
  set page(paper: paper, margin: (x: margin-x, y: margin-y))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  if title != none { title-block(title, author) }
  body
}
