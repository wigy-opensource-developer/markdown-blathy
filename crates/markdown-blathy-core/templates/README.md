# MarkdownBláthy templates

This directory holds the bundled Typst templates, one subdirectory per
template, each with a `main.typ` and — optionally — its own fonts and images:

```
templates/
├── prelude.typ          # shared helper functions (NOT a template)
├── light/main.typ       # the default light theme
├── dark/main.typ        # white-on-black GitHub-dark theme
└── obsidian/
    ├── main.typ         # colorful dark "Obsidian PKM" theme
    └── assets/corner.svg
```

A *custom* user template (picked via the Page settings "Choose…" button) lives
wherever the user put it and does **not** need to follow this layout — this
file describes the contract every template (bundled or custom) relies on.

## How a template is composed

Every render is compiled from three pieces, concatenated by the Rust side:

```
prelude  +  template  +  invocation
```

1. **`prelude.typ`** — default implementations of every helper the
   markdown→Typst converter emits (tables, task lists, code, links, …) plus
   the font pinning rules. It does **not** define `article`.
2. **The template** — the selected `main.typ` (bundled) or the user's `.typ`
   file (custom). It is concatenated **after** the prelude, so Typst
   *shadowing* applies: any function the template redefines wins; everything
   it leaves out falls back to the prelude default.
3. **The invocation** — Rust appends a single call:

   ```typ
   #article(
     paper: "a4",
     margin-x: 20mm,
     margin-y: 25mm,
     text-align: "left",
     title: sys.inputs.title,
     author: sys.inputs.author,
   )[
     #eval(sys.inputs.body, mode: "markup", scope: (<helpers>))
   ]
   ```

   `title`, `author` and `body` arrive via `sys.inputs` per render; the
   toolbar options (`paper`, `margin-x`, `margin-y`, `text-align`) are baked
   into the invocation as literal arguments.

### Bundled vs. custom templates

| | Composed as | Must define `article`? |
|---|---|---|
| Bundled (`templates/<name>/main.typ`) | `prelude + template` | **Yes** — there is no default article prepended. |
| Custom (user `.typ` file) | `prelude + light/article + template` | No — the light theme's `default-article` (aliased as `article`) is the fallback. |

The bundled `light/main.typ` keeps `default-article` reachable under its own
name so other templates can *wrap* the default look instead of replacing it:

```typ
#let article(..args) = { /* tweaks */ default-article(..args) }
```

## The required entry point: `article`

Every template must make an `article` function reachable. Its signature is
fixed by the invocation above — parameter names matter:

```typ
#let article(
  paper: "a4",        // Typst paper name: "a4" | "a5" | "us-letter" | "us-legal"
  margin-x: 20mm,     // smaller of the left/right margins
  margin-y: 25mm,     // smaller of the top/bottom margins
  text-align: "left", // "left" | "center" | "justified"
  title: none,        // string or none (from front matter)
  author: none,       // string or none (from front matter)
  body,               // the converted, escaped markdown as content
) = {
  set page(paper: paper, margin: (x: margin-x, y: margin-y))
  // …your styling…
  if title != none { title-block(title, author) }
  body
}
```

The markdown body is *escaped* by the converter before it reaches `body`, so
it is safe to place it in a content block; you never see raw markdown.

## Converter contract

This is what the markdown→Typst converter emits for each construct. "Helper"
columns name the prelude function the template may override; constructs marked
*native* use built-in Typst and are themeable with `#show` rules instead.

| Markdown | Emitted Typst | Helper to override |
|---|---|---|
| `# Heading` | `= Heading #label("slug")` | — (native; `#show heading`) |
| `**bold**`, `*em*`, `~~s~~` | `*bold*`, `_em_`, `#strike[s]` | — (native) |
| `` `code` `` | `#inline-code("code")` | `inline-code` |
| ```` ```lang ``` ```` | `#code-block("lang", "source")` | `code-block` |
| `[text](https://…)` | `#link("https://…")[text]` | — (native; `#show link`) |
| `[text](#fragment)` | `#internal-link("fragment")[text]` | `internal-link` |
| `![alt](img.png)` | `#figure-image("img.png", [alt])` | `figure-image` |
| `- item` | native `- item` | — (native; `#show list`) |
| `1. item` | `#ordered-list(start, [item], …)` | `ordered-list` |
| `- [ ] task` | `#task-list(task-done([…]), task-todo([…]), task-plain([…]))` | `task-list`, `task-done`, `task-todo`, `task-plain` |
| `> quote` | `#blockquote[…children…]` | `blockquote` |
| `\| a \| b \|` table | `#markdown-table(columns: N, header: ([a], [b]), rows: (([1], [2]), …))` | `markdown-table` |
| `---` rule | `#thematic-break()` | `thematic-break` |
| `==mark==` | `#mark[m]` | `mark` |
| `^sup^`, `~sub~` | `#superscript[sup]`, `#subscript[sub]` | `superscript`, `subscript` |
| term / definition | `#definition-list(([term], [details]), …)` | `definition-list` |
| `[^1]` footnote | `#footnote[…note…]` | — (native) |
| `[[_TOC_]]` | `#markdown-toc(((level, text, slug), …))` | `markdown-toc` |
| `<!-- pagebreak -->` | `#pagebreak()` | — (native) |
| long unbreakable token (24+ chars) | `#break-long("token")` | `break-long` |
| `title:` / `author:` front matter | `sys.inputs.title` / `sys.inputs.author` | `title-block` |

## Helper reference

The prelude defines these functions with the following shapes (override any of
them with a `#let` of the same name and parameters):

| Helper | Signature | Purpose |
|---|---|---|
| `task-list` | `#let task-list(..items)` | Concatenates `(glyph, content)` pairs into a two-column grid. |
| `task-done` / `task-todo` / `task-plain` | `#let task-done(body)` | Return `(checkbox-cell, content-cell)` pairs. |
| `blockquote` | `#let blockquote(body)` | Wraps the native `quote(block: true)`. |
| `thematic-break` | `#let thematic-break()` | Horizontal rule spacing + stroke. |
| `inline-code` | `#let inline-code(text)` | Inline `raw` styling. |
| `code-block` | `#let code-block(lang, source)` | Block `raw` chrome (fill, inset, radius). |
| `markdown-table` | `#let markdown-table(columns: 1, header: (), rows: ())` | Emits one `auto` column per markdown column, bolds the header. |
| `definition-list` | `#let definition-list(..items)` | `(term, details)` pairs in a two-column grid. |
| `ordered-list` | `#let ordered-list(start, ..items)` | Wraps native `enum(start:, numbering: "1.")`. |
| `figure-image` | `#let figure-image(url, caption)` | Image width + caption placement. |
| `internal-link` | `#let internal-link(fragment, body)` | Guarded link to a heading label. |
| `mark` / `superscript` / `subscript` | `#let mark(body)` … | Thin wrappers over `highlight` / `super` / `sub`. |
| `break-long` | `#let break-long(s)` | Inserts invisible break opportunities in long tokens. |
| `markdown-toc` | `#let markdown-toc(items)` | Renders the collected `(level, text, slug)` headings. |
| `title-block` | `#let title-block(title, author)` | Front-matter title/author styling (called by `article`). |

`pagebreak` and `footnote` are native Typst, not prelude helpers.

## `sys.inputs`

Three keys are injected for every render:

- `sys.inputs.title` — front-matter title as a string, or `none`.
- `sys.inputs.author` — front-matter author as a string, or `none`.
- `sys.inputs.body` — the converted, escaped markdown as a **string** (the
  invocation above `eval`s it in markup mode). Do not edit it.

## Fonts and images next to the template

- **Fonts** — a template's own folder is scanned recursively for font files,
  so a theme can ship a `.ttf`/`.otf` next to `main.typ`. Reference it the
  usual way: `#set text(font: "Family Name")`. Only the families a template
  references via `font:` rules are loaded from the system (plus the embedded
  DejaVu Sans/Sans Mono/Math faces); a `#let`-indirect font falls back to the
  full system scan.
- **Images** — the template's folder is also a file-system root, so a
  relative `image("assets/bg.svg")` resolves next to `main.typ`. Both vector
  (SVG) and bitmap (PNG/JPG) go through the same `image()` path. Document
  images resolve against the *document's* folder and take precedence on name
  collisions. Remote images are unsupported.

## Validation

A custom template is accepted only when a probe document compiles against it.
The probe exercises every converter helper (except `figure-image`), so a
template that only restyles — or defines nothing at all — passes, while a
syntax error or a *broken override* (e.g. a `task-list` that calls an
undefined function) is rejected and the previous template is kept.

## Minimal template

```typ
// Composed after the prelude; anything not redefined keeps the prelude default.
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
  show heading: set text(fill: rgb("#ffffff"))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  if title != none { title-block(title, author) }
  body
}
```

See `dark/main.typ` and `obsidian/main.typ` for complete worked examples.
