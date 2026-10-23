// obsidian/main.typ — a colorful dark "Obsidian PKM" theme for MarkdownBláthy.
//
// Composed AFTER the prelude (§6.3), so it overrides the same seams the dark
// theme does, but with a purple graph palette and a small linked-nodes SVG in
// the lower-right corner of every page (shipped next to this file under
// `assets/` and resolved through the template's asset root).
//
// Overrides:
//   article        — purple page fill, light text, corner decoration
//   title-block    — front-matter title/author styling
//   task-*         — colored task bullets
//   blockquote, thematic-break, inline-code, code-block, markdown-table, mark

// --- Page + text: purple-black with colorful accents --------------------------
#let article(
  paper: "a4",
  margin-x: 20mm,
  margin-y: 25mm,
  text-align: "left",
  title: none,
  author: none,
  body,
) = {
  set page(
    paper: paper,
    margin: (x: margin-x, y: margin-y),
    fill: rgb("#191724"),
    // The "linked notes" motif lives in the page background, so it repeats on
    // every page without disturbing the text flow. `bottom + right` pins it
    // to the lower-right corner; dx/dy pull it a little inside the edge.
    background: place(
      bottom + right,
      dx: 10pt,
      dy: 10pt,
      image("assets/corner.svg", width: 22pt),
    ),
  )
  set text(fill: rgb("#e0def4"))
  set par(justify: false)
  // Footnote divider: visible on the dark page (the stock one is black).
  set footnote.entry(separator: line(length: 30%, stroke: 0.5pt + rgb("#524f67")))
  // Obsidian-ish accents: lavender links, teal headings.
  show link: set text(fill: rgb("#c4a7e7"))
  show heading: set text(weight: 600, fill: rgb("#9ccfd8"))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  // Center TOC/definition blocks as a unit when text-align is center.
  show block.where(inset: 10pt, radius: 6pt): it => if text-align == "center" { align(center, block(width: auto, it)) } else { it }
  if title != none { title-block(title, author) }
  body
}

// --- Title/author from YAML front matter --------------------------------------
#let title-block(title, author) = {
  set align(center)
  text(size: 1.6em, weight: 700, fill: rgb("#ffffff"))[#title]
  if author != none {
    linebreak()
    text(size: 1.1em, weight: 500, fill: rgb("#908caa"))[#author]
  }
  v(0.8em)
}

// --- Task items: colorful state bullets ---------------------------------------
// Each helper returns a (checkbox cell, content cell) pair for `task-list`'s
// two-column grid. Glyphs stay pinned to the embedded DejaVu Sans (same as the
// prelude) so the ballot/circle codepoints render without the slow font-fallback
// search.
#let task-done(body) = (
  [#text(font: "DejaVu Sans")[#text(fill: rgb("#9ccfd8"))[#sym.checkmark]]],
  [#text(fill: rgb("#6e6a86"))[#body]], // done items read muted
)
#let task-todo(body) = (
  [#text(font: "DejaVu Sans")[#text(fill: rgb("#c4a7e7"))[#sym.circle]]],
  [#body],
)
#let task-plain(body) = (
  [#text(font: "DejaVu Sans")[–]],
  [#body],
)

// --- Block quotes --------------------------------------------------------------
#let blockquote(body) = block(
  fill: rgb("#1f1d2e"),
  stroke: (left: 0.5pt + rgb("#7c3aed")),
  inset: (x: 10pt, y: 6pt),
  radius: 4pt,
  [#body],
)

// --- Thematic break --------------------------------------------------------------
#let thematic-break() = [#v(0.6em) #line(length: 100%, stroke: 0.5pt + rgb("#524f67")) #v(0.6em)]

// --- Inline code & code blocks -----------------------------------------------------
#let inline-code(code) = text(fill: rgb("#eb6f92"))[#raw(code)]
#let code-block(lang, source) = block(
  fill: rgb("#1f1d2e"),
  inset: 8pt,
  radius: 4pt,
  raw(source, block: true, lang: if lang == "" { none } else { lang }),
)

// --- Highlight (==mark==): purple card behind light text --------------------------
#let mark(body) = highlight(fill: rgb("#2a273f"), body)

// --- Tables: keep the prelude's auto columns + bold header, violet lines ---------
#let markdown-table(columns: 1, header: (), rows: ()) = {
  let cells = ()
  for row in rows { cells = cells + row }
  let widths = ()
  for i in range(columns) { widths = widths + (auto,) }
  table(
    columns: widths,
    stroke: 0.5pt + rgb("#524f67"),
    ..header.map(cell => [*#cell*]),
    ..cells.map(cell => [#cell]),
  )
}
