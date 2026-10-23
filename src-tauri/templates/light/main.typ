// light/main.typ — the default light theme for MarkdownBláthy.
//
// Composed AFTER the prelude (§6.3). It provides the default `article` entry
// point (page setup, heading/link styling, center-mode handling) and keeps
// `default-article` reachable under its own name so other templates can wrap
// the default look instead of replacing it wholesale.

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
#let article = default-article
