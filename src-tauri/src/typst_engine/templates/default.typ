// default.typ — embedded prelude for MarkdownBláthy.
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
//   break-long(text)                 // text is a string; adds break opportunities
//   code-block(lang, source)         // lang/source are strings
//   markdown-table(header, rows)     // header: array of content, rows: array of arrays
//   ordered-list(start, ..items)     // native Typst enum, markdown-style markers
//   internal-link(fragment)[body]    // guarded link to a heading label
//   title-block(title, author)       // called by the default article
//   article(…)                       // the invocation the Rust side appends

// --- Default fonts -------------------------------------------------------------
// Top-level (not inside the article): this runs BEFORE a user template, so a
// template's own `#set text(font: ...)` — which comes later — wins over this
// default, while templates that set no font keep DejaVu Sans (M6).
#set text(font: "DejaVu Sans", size: 11pt)

// Code (`raw`) is pinned to the embedded monospace face: the default
// template's font book loads no system fonts, so `raw()`'s DejaVu Sans Mono
// default must be embedded and named here, or it silently falls back to the
// proportional body font — breaking code indentation and word spacing.
#show raw: set text(font: "DejaVu Sans Mono")

// Math is pinned to the embedded math face: Typst resolves math glyphs only
// from a MATH-table OpenType font, and none of the text faces provide one —
// without this rule every formula fails with "no font could be found" when
// the font book holds no other math font (the default book loads no system
// fonts). A theme can restyle equations with its own `#show math.equation`
// rule placed after this prelude.
#show math.equation: set text(font: "DejaVu Math TeX Gyre")

// --- Task lists --------------------------------------------------------------
// Each item function returns a (checkbox cell, content cell) pair; the grid
// needs those as flat alternating cells, so `task-list` concatenates the
// pairs (`.pos()` because variadic arguments are not iterable directly).
//
// The checkbox glyphs are pinned to the embedded DejaVu Sans: many fonts (e.g.
// Ubuntu) lack the ballot-box codepoints (U+2610/U+2611), and letting Typst
// fall back across the system font book is ~10x slower per render AND yields a
// solid box instead of a hollow one. Pinning keeps the checkbox hollow and
// fast in every theme; it still renders in the current text colour.
#let task-done(body) = ([#text(font: "DejaVu Sans")[#sym.ballot.check]], [#body])
#let task-todo(body) = ([#text(font: "DejaVu Sans")[#sym.ballot]], [#body])
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

// --- Images: default width + caption placement (overridable) ------------------
#let figure-image(url, caption) = figure(image(url, width: 80%), caption: caption)

// --- Thematic break ----------------------------------------------------------
#let thematic-break() = [#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)]

// --- Inline code (string argument avoids backtick-fence escaping) ------------
#let inline-code(text) = raw(text)

// --- Long unbreakable words --------------------------------------------------
// Typst's line breaker follows Unicode UAX #14: it breaks at spaces and soft
// punctuation, but a long run with none of those (a chemical formula, a hash)
// has no break opportunity and crosses the margin. The converter hands every
// token of ~24+ characters to this helper. A zero-width `box` is an inline
// item, and Typst offers a contingent line-break opportunity around inline
// items — so the boxes make the token wrappable without being visible and
// without polluting the PDF text layer (unlike a zero-width space), keeping
// copy/search intact. Breaks are preferred after separators; the 12-cluster
// fallback lets even a featureless run wrap. Templates may override this
// (e.g. `#let break-long(s) = s` to turn it off).
#let break-long(s) = {
  let out = []
  let run = 0
  for c in s.clusters() {
    out += c
    run += 1
    if run >= 12 or c in ("=", ")", "(", "/", "-", ",", ";", ".", ":") {
      out += box(width: 0pt)
      run = 0
    }
  }
  out
}

// --- Highlight / superscript / subscript (==mark==, ^sup^, ~sub~) ------------
// Thin wrappers over the Typst built-ins so a custom template can restyle
// them by redefining the helper (§6.3).
#let mark(body) = highlight(body)
#let superscript(body) = super(body)
#let subscript(body) = sub(body)

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

// --- Definition lists: (term, details) pairs in a two-column grid -----------
// Typst has no native definition list, so each `(term, details)` tuple the
// converter passes becomes a row: bold term on the left, details on the right.
#let definition-list(..items) = {
  let cells = ()
  for item in items.pos() {
    cells.push(strong(item.at(0)))
    cells.push(item.at(1))
  }
  grid(
    columns: (auto, 1fr),
    column-gutter: 12pt,
    row-gutter: 0.65em,
    ..cells,
  )
}

// --- Ordered lists: native enum with markdown-style "1." markers -------------
#let ordered-list(start, ..items) = enum(start: start, numbering: "1.", ..items)

// --- Internal links: `[text](#fragment)` → a link to the heading label -------
// The converter emits `internal-link(fragment)` for `#fragment` URLs. The
// guard keeps a broken TOC entry from failing the render: `query` needs a
// known context, and a missing label degrades to plain text — the lossy
// philosophy the converter applies to HTML and dangling footnotes (§6.2).
// In typst 0.15 the SVG preview cannot resolve Location links to `href`s
// (the paged introspector has no anchors), so preview links are inert;
// PDF links work. When typst grows paged anchors, this helper needs no
// change — the resolution happens in the renderer.
#let internal-link(fragment, body) = context {
  if query(label(fragment)).len() > 0 {
    link(label(fragment), body)
  } else {
    body
  }
}

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
//
// The text-align set rules are UNCONDITIONAL with a conditional *value* — a
// `set` rule scoped to an `if` body would only affect that (empty) body and
// never reach `body` (deferred issue §13: the Align control rendered
// identically for all three choices).
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
  set align(if text-align == "center" { center } else { left })
  set par(justify: text-align == "justified")
  show heading: set text(weight: 600)
  show link: set text(fill: rgb("#0366d6"))
  // Center mode: a list must behave as ONE block — items left-aligned
  // (bullets adjacent to their text, marker and text travel together) and
  // the whole block centered. Without these rules `set align(center)`
  // centers each text line independently while the bullets stay pinned to
  // the left margin (deferred issue §13). `center-block` shrinks to the
  // longest line so the centering is visible, and the inner `set
  // align(left)` restores item-level left alignment. Left/justified modes
  // pass the content through unchanged. (The one-line `[#set align(left)
  // #content]` form is a Typst parse error — `#` is not valid in code —
  // hence the multi-line block.)
  let center-block(content) = align(center, block(width: auto)[
    #set align(left)
    #content
  ])
  show list: it => if text-align == "center" { center-block(it) } else { it }
  show enum: it => if text-align == "center" { center-block(it) } else { it }
  // Task lists are grids with `(auto, 1fr)` columns. A plain `show grid`
  // rule would recurse on its own output, so target exactly that shape and
  // rebuild it with `(auto, auto)` columns (which no longer matches the
  // selector) inside the same centered block. This only fires on grids of
  // exactly that shape, i.e. task lists.
  show grid.where(columns: (auto, 1fr)): it => if text-align == "center" {
    center-block(
      grid(
        columns: (auto, auto),
        column-gutter: it.column-gutter,
        row-gutter: it.row-gutter,
        ..it.children,
      )
    )
  } else { it }
  show block.where(inset: 10pt, radius: 6pt): it => if text-align == "center" { center-block(it) } else { it }
  if title != none { title-block(title, author) }
  body
}

// Public name; user templates may redefine it to replace the whole look.

// --- Table of contents: `[[_TOC_]]` → #markdown-toc(...) -------------------
// The converter collects headings as `(level, text, slug)` tuples and passes
// them as a list argument. This helper renders indented links — no
// auto-numbering (the heading text carries whatever the author wrote).
// Template authors can override the function to produce a completely
// different layout (sidebar, horizontal, numbered, …).
#let markdown-toc(items) = {
  let depth = 3
  let filtered = ()
  for item in items {
    if item.at(0) <= depth and item.at(2).len() > 0 {
      filtered = filtered + (item,)
    }
  }
  let toc-content = ""
  for item in filtered {
    let level = item.at(0)
    let text = item.at(1)
    let slug = item.at(2)
    let indent = ""
    for j in range(level - 1) {
      indent = indent + "  " 
    }
    toc-content = toc-content + [#(indent)#link(label(slug), text) #linebreak()]
  }
  if filtered.len() > 0 {
    block(
      inset: 10pt,
      radius: 6pt,
      toc-content
    )
  }
}
#let article = default-article
