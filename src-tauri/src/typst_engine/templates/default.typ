// default.typ — embedded prelude for tauri-md-editor.
//
// This file is the PRELUDE shared by every template (§6.3): it provides
// default implementations of the helper functions the markdown→Typst
// converter emits, plus a default `article`. A user template is composed
// AFTER this prelude, so Typst shadowing applies: any function the user
// redefines wins, everything else falls back to these defaults.
//
// Converter contract (each function the converter may call):
//   task-list(task-done(body), task-todo(body), task-plain(body), …)
//   blockquote(body)
//   thematic-break()
//   inline-code(text)                // text is a string
//   code-block(lang, source)         // lang/source are strings
//   markdown-table(header, rows)     // header: array of content, rows: array of arrays
//   ordered-list(start, ..items)     // native Typst enum, markdown-style markers
//   title-block(title, author)       // called by the default article
//   article(…)                       // the invocation the Rust side appends

// --- Task lists --------------------------------------------------------------
// Each item function returns a (checkbox cell, content cell) pair; the grid
// needs those as flat alternating cells, so `task-list` concatenates the
// pairs (`.pos()` because variadic arguments are not iterable directly).
#let task-done(body) = ([#sym.ballot.check], [#body])
#let task-todo(body) = ([#sym.ballot], [#body])
#let task-plain(body) = ([•], [#body])
#let task-list(..items) = {
  let cells = ()
  for item in items.pos() { cells = cells + item }
  grid(
    columns: (auto, 1fr),
    column-gutter: 8pt,
    row-gutter: 0.65em, // matches default par.leading: uniform line rhythm
    ..cells,
  )
}

// --- Block quotes (native Typst `quote` since 0.13) --------------------------
#let blockquote(body) = quote(block: true)[#body]

// --- Thematic break ----------------------------------------------------------
#let thematic-break() = [#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)]

// --- Inline code (string argument avoids backtick-fence escaping) ------------
#let inline-code(text) = raw(text)

// --- Code blocks: light background, monospace, no highlighting in v1 ---------
#let code-block(lang, source) = block(
  fill: rgb("#f5f5f5"),
  inset: 8pt,
  radius: 4pt,
  raw(source, block: true, lang: if lang == "" { none } else { lang }),
)

// --- Markdown tables: auto columns, bold header ------------------------------
// `columns` is the markdown column count, passed by the converter. A bare
// `columns: auto` would be a *single* auto column and stack every cell
// vertically, so one `auto` width is emitted per column (§6.2).
#let markdown-table(columns: 1, header: (), rows: ()) = {
  let cells = ()
  for row in rows { cells = cells + row }
  let widths = ()
  for i in range(columns) { widths = widths + (auto,) }
  table(
    columns: widths,
    ..header.map(cell => [*#cell*]),
    ..cells.map(cell => [#cell]),
  )
}

// --- Ordered lists: native enum with markdown-style "1." markers -------------
#let ordered-list(start, ..items) = enum(start: start, numbering: "1.", ..items)

// --- Title/author block from YAML front matter (§6.2) -------------------------
#let title-block(title, author) = {
  set align(center)
  text(size: 1.6em, weight: 700)[#title]
  if author != none {
    linebreak()
    text(size: 1.1em, weight: 500)[#author]
  }
  v(0.8em)
}

// --- Default article ----------------------------------------------------------
// The `set`/`show` rules live inside the function so a template that overrides
// `article` entirely escapes the default look. `default-article` stays
// reachable under its own name so overriders can wrap it.
#let default-article(
  paper: "a4",
  margin-x: 20mm,
  margin-y: 25mm,
  text-align: "left",
  title: none,
  author: none,
  body,
) = {
  set page(paper: paper, margin: (x: margin-x, y: margin-y))
  set text(font: "DejaVu Sans", size: 11pt)
  set par(justify: false)
  show heading: set text(weight: 600)
  show link: set text(fill: rgb("#0366d6"))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  if title != none { title-block(title, author) }
  body
}

// Public name; user templates may redefine it to replace the whole look.
#let article = default-article
