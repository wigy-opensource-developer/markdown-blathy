// dark.typ — example dark theme for tauri-md-editor.
//
// Load it via the toolbar's Template… button to see how the prelude (§6.3)
// lets a theme restyle the document WITHOUT touching the markdown→Typst
// converter. This file is composed AFTER the embedded prelude, so each
// definition below overrides the matching default helper; anything not
// defined here keeps the prelude implementation (Typst shadowing).
//
// Overrides shown (each is a seam a theme designer can replace):
//   article          — page fill (black) + light text + link/heading colors
//   title-block      — front-matter title/author styling
//   task-done/task-todo/task-plain — alternative task item bullets
//   blockquote, thematic-break, inline-code, code-block, markdown-table, mark

// --- Page + text: white on black ---------------------------------------------
// The `article` function is the only required seam. It receives the render
// options as named arguments (paper, margin-x, margin-y, text-align, title,
// author) plus the converted markdown as `body`.
#let article(
  paper: "a4",
  margin-x: 20mm,
  margin-y: 25mm,
  text-align: "left",
  title: none,
  author: none,
  body,
) = {
  set page(paper: paper, margin: (x: margin-x, y: margin-y), fill: rgb("#0d1117"))
  set text(fill: rgb("#e6edf3"))
  set par(justify: false)
  // Footnote divider: the stock separator is a black line that vanishes on a
  // dark page, so restyle it via footnote.entry (its separator/clearance/gap
  // are settable — the seam for footnote theming).
  set footnote.entry(separator: line(length: 30%, stroke: 0.5pt + rgb("#30363d")))
  // GitHub-dark-ish link and heading colors.
  show link: set text(fill: rgb("#58a6ff"))
  show heading: set text(weight: 600, fill: rgb("#ffffff"))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  if title != none { title-block(title, author) }
  body
}

// --- Title/author from YAML front matter --------------------------------------
#let title-block(title, author) = {
  set align(center)
  text(size: 1.6em, weight: 700, fill: rgb("#ffffff"))[#title]
  if author != none {
    linebreak()
    text(size: 1.1em, weight: 500, fill: rgb("#8b949e"))[#author]
  }
  v(0.8em)
}

// --- Alternative task item bullets ---------------------------------------------
// Each helper returns a (checkbox cell, content cell) pair; `task-list`
// concatenates them into the two-column grid. Override them to swap the
// glyphs and/or restyle the item text. The glyphs are pinned to the embedded
// DejaVu Sans (like the prelude) so they never trigger the slow font-fallback
// search — the fill still comes from the theme.
#let task-done(body) = (
  [#text(font: "DejaVu Sans")[#text(fill: rgb("#3fb950"))[#sym.checkmark]]],
  [#text(fill: rgb("#7d8590"))[#body]], // done items read muted
)
#let task-todo(body) = (
  [#text(font: "DejaVu Sans")[#text(fill: rgb("#8b949e"))[#sym.circle]]],
  [#body],
)
#let task-plain(body) = (
  [#text(font: "DejaVu Sans")[–]],
  [#body],
)

// --- Block quotes --------------------------------------------------------------
#let blockquote(body) = block(
  fill: rgb("#161b22"),
  inset: (x: 10pt, y: 6pt),
  radius: 4pt,
  [#body],
)

// --- Thematic break --------------------------------------------------------------
#let thematic-break() = [#v(0.6em) #line(length: 100%, stroke: 0.5pt + rgb("#30363d")) #v(0.6em)]

// --- Inline code & code blocks -----------------------------------------------------
// Note: the parameter is named `code` (not `text`) so it does not shadow the
// Typst `text` function used to color the run.
#let inline-code(code) = text(fill: rgb("#ff7b72"))[#raw(code)]
#let code-block(lang, source) = block(
  fill: rgb("#161b22"),
  inset: 8pt,
  radius: 4pt,
  raw(source, block: true, lang: if lang == "" { none } else { lang }),
)

// --- Highlight (==mark==): a dark derivative of the inline-code accent -------------
// The prelude's stock `highlight` fill is a light yellow designed for white
// pages — light text on it would be unreadable here. Scaling each channel of
// the theme's warm accent (#ff7b72) to one third gives #552926, a deep brick
// card that stays subtle on the near-black page while keeping the text light.
#let mark(body) = highlight(fill: rgb("#552926"), body)

// --- Tables: keep the prelude's auto columns + bold header, add dark lines --------
#let markdown-table(columns: 1, header: (), rows: ()) = {
  let cells = ()
  for row in rows { cells = cells + row }
  let widths = ()
  for i in range(columns) { widths = widths + (auto,) }
  table(
    columns: widths,
    stroke: 0.5pt + rgb("#30363d"),
    ..header.map(cell => [*#cell*]),
    ..cells.map(cell => [#cell]),
  )
}
