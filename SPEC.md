# Tauri MD Editor — Implementation Specification

| | |
|---|---|
| **Status** | Draft v1.0 |
| **Target** | Desktop app (Windows / macOS / Linux) built with Tauri 2 + Rust + Typst |
| **Reader** | AI agent or human implementer. Assumes solid Rust & Tokio; beginner–intermediate Tauri 2 and HTML/CSS. Tauri- and CSS-specific concepts are explained inline where they first appear. |

---

## 1. Product overview

A desktop **Markdown editor with live preview and on-demand PDF export**. The preview and the exported PDF are produced by **the same Typst compilation pipeline**, which guarantees the preview is pixel-identical to the PDF (true WYSIWYG).

User-visible behaviour:

1. Split-screen window: **Markdown editor** (left) and **live preview** (right).
2. The preview updates automatically while typing (debounced), rendering pages with print margins, exactly as they will appear in the PDF.
3. A toolbar offers: New / Open / Save Markdown files, flavor selection (CommonMark / GitHub Flavored Markdown / Custom), paper size, margins, text alignment, and **Export PDF** (system save dialog).
4. The default document style is provided by an embedded Typst template; users may supply their own `.typ` template.
5. Compile warnings (e.g. unknown fonts) are surfaced in the UI, not swallowed.

## 2. Non-goals (v1)

- No incremental compilation (tinymist-style). Full recompile per change is fine for typical documents (< ~50 pages).
- No client-side Typst WASM (`typst.ts`). All compilation happens in Rust; this avoids version drift between preview and PDF.
- No syntax highlighting inside code blocks in the PDF (Typst has no built-in highlighter; see §13).
- No raw-HTML support in Markdown (lossy by design, see §6.2).
- No remote image fetching.
- No mobile support.

## 3. Architecture

```
┌─────────────────────────── WebView (frontend) ───────────────────────────┐
│  React 19 + Mantine 8                                                  │
│  CodeMirror 6 (markdown, via @uiw/react-codemirror) → debounce 200ms    │
│      → invoke("render_preview")                                         │
│  Preview pane: <div> of inline SVG pages (one per printed page)          │
│  Mantine Toolbar / status bar / error banner                            │
└───────────────▲──────────────────────────────────────────┬───────────────┘
        invoke()│  (JSON over Tauri IPC)                    │invoke()
┌───────────────┴──────────────────────────────────────────▼───────────────┐
│  src-tauri (Rust) — thin command layer (tauri::command)                  │
│      │                                                                   │
│      │  all real logic lives in plain modules (no tauri imports):        │
│      ▼                                                                   │
│  markdown::convert  comrak → AST → Typst markup (escaped)                │
│  typst_engine::template   compose prelude + user template + invocation   │
│  typst_engine::engine     TypstEngine (typst-as-lib) → compile           │
│      ├── typst_svg  → per-page SVG strings  (preview)                    │
│      └── typst_pdf  → PDF bytes             (export)                     │
└──────────────────────────────────────────────────────────────────────────┘
```

**Design rule (non-negotiable):** `markdown/` and `typst_engine/` must not import `tauri`. The `#[tauri::command]` functions are thin, serialization-only wrappers. This is what makes the core unit-testable without a running app.

**Commands are stateless in v1** — each invoke recompiles the document from the current inputs. Perf is acceptable: Typst compiles a typical page in single-digit milliseconds, and the M6 engine cache + memoization keep per-render cost low (see §7.5). Incremental compilation is future work (§13).

## 4. Tech stack & versions

### 4.1 Rust (`src-tauri/Cargo.toml`)

```toml
[package]
name = "tauri-md-editor"
version = "0.1.0"
description = "Markdown editor with Typst-powered live preview and PDF export"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-dialog = "2"
open = "5"               # open exported PDFs in the system viewer (M4)

serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"

comrak = "0.54"             # CommonMark/GFM parser
typst = "0.15"              # the Typst compiler (library)
typst-layout = "0.15"       # the paged document type (re-exported as typst_layout::PagedDocument)
typst-pdf = "0.15"          # PDF export backend
typst-svg = "0.15"          # SVG export backend (preview pages)
typst-as-lib = "0.16"       # high-level TypstWorld wrapper (avoids hand-writing World)
```

**Version pinning rule:** `typst`, `typst-layout`, `typst-pdf`, `typst-svg` must **all share the same minor version** (they are released in lockstep). `typst-as-lib` follows its own versioning but pins `typst ^0.15` — 0.16 is the first line compatible with Typst 0.15 (0.15.x of typst-as-lib pins Typst 0.14; do not mix them). Pin exact versions in `Cargo.lock`. **MSRV:** Typst 0.15 requires **Rust ≥ 1.92**. Before implementing, read the docs.rs pages for the four crates at the pinned version — the API names below are correct for the 0.15/0.16 line but you must confirm signatures against the exact version you lock (see §6.4).

> **Note on comrak's built-in Typst formatter:** comrak briefly shipped a `format_typst` / `markdown_to_typst` API (0.51.0) but **removed it again in 0.52.0** after the code turned out to be LLM-generated (comrak CHANGELOG, PR #781). Do **not** rely on it. The custom converter in §6.2 is the supported path.

### 4.2 Frontend (`package.json`)

React 19 + Mantine 8, bundled by Vite. No CSS framework beyond Mantine's own styles. No frontend Markdown renderer — the preview is Typst SVG, not HTML. The editor is CodeMirror 6 wrapped for React via `@uiw/react-codemirror`.

```json
{
  "name": "tauri-md-editor",
  "private": true,
  "version": "0.1.0",
  "license": "GPL-3.0-or-later",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "check:licenses": "node scripts/check-licenses.mjs"
  },
  "dependencies": {
    "@fontsource/iosevka": "^5",
    "@fontsource-variable/archivo-narrow": "^5",
    "@mantine/core": "^8",
    "@mantine/hooks": "^8",
    "@tabler/icons-react": "^3",
    "@tauri-apps/api": "^2",
    "@uiw/react-codemirror": "^4.25",
    "@codemirror/lang-markdown": "^6",
    "react": "^19",
    "react-dom": "^19"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@vitejs/plugin-react": "^4",
    "vite": "^6"
  }
}
```

The `@codemirror/*` packages come transitively through `@uiw/react-codemirror`; add `@codemirror/lang-markdown` (the markdown language extension) explicitly. Fonts are self-hosted via `@fontsource` (bundled by Vite, no external requests). `scripts/check-licenses.mjs` enforces an SPDX allow-list on the frontend dependency tree (part of `./check.sh`).

### 4.3 System prerequisites (per platform)

- **Rust** ≥ 1.92 (Typst 0.15 MSRV).
- **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, plus a C linker and `pkg-config` (see Tauri docs). `build-essential`, `libssl-dev` may also be required.
- **macOS**: Xcode Command Line Tools.
- **Windows**: WebView2 runtime (preinstalled on Windows 11 / updated Windows 10).

## 5. Repository structure

```
tauri-md-editor/
├── package.json
├── vite.config.js               # port 1420, strictPort (standard Tauri template)
├── index.html                   # single page: <div id="root"> + module script
├── scripts/check-licenses.mjs   # SPDX allow-list check for frontend deps
├── src/                         # ── frontend (React 19 + Mantine 8) ──
│   ├── main.jsx                 # bootstrap: MantineProvider + <App/>
│   ├── App.jsx                  # layout shell: toolbar + panes + status bar
│   ├── components/
│   │   ├── Toolbar.jsx          # New/Open/Save, flavor/paper/margins/align,
│   │   │                        #   Template…, Export PDF
│   │   ├── Editor.jsx           # CodeMirror 6 via @uiw/react-codemirror
│   │   ├── Preview.jsx          # SVG page rendering, stale-response guard
│   │   ├── StatusBar.jsx        # page count / warnings / Ready
│   │   └── ErrorBanner.jsx      # transient error display
│   ├── lib/
│   │   ├── api.js               # thin invoke() wrappers + payload types
│   │   ├── settings.js          # localStorage-backed settings (useSettings hook)
│   │   └── presets.js           # flavor/paper/margin/align option tables
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs                 # tauri_build::build()
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── icons/                   # generated by `tauri icon`
│   └── src/
│       ├── main.rs              # calls tauri_md_editor_lib::run()
│       ├── lib.rs               # Builder, state, command registration
│       ├── commands.rs          # thin #[tauri::command] wrappers
│       ├── options.rs           # PdfOptions, Flavor, margins, serde types
│       ├── markdown/
│       │   ├── mod.rs
│       │   ├── convert.rs       # ★ comrak AST → Typst markup (the core)
│       │   └── escape.rs        # Typst markup escaping helpers
│       ├── typst_engine/
│       │   ├── mod.rs
│       │   ├── template.rs      # prelude embedding, compose(), static source + inputs
│       │   ├── engine.rs        # TypstEngine build, compile → SVG pages / PDF
│       │   └── templates/
│       │       └── default.typ  # embedded prelude (default helpers + article)
│       └── fonts/               # embedded DejaVu Sans (regular + bold, §6.3)
└── tests/                       # optional: integration tests (plain Rust)
```

**Tauri 2 concept (one paragraph):** the app is two processes glued by IPC. The Rust side is the "main" process; the UI runs inside the platform's webview and talks to Rust only through `invoke()` calls to functions annotated `#[tauri::command]`, plus events for push. Permissions ("capabilities") gate which commands/plugins the webview may call. In dev, `tauri dev` starts Vite (hot reload) and feeds the webview from `devUrl`; in production the webview loads the built static files from `frontendDist`. The webview has **no** Node integration by default — that is the secure baseline and must not be disabled.

## 6. Core pipeline

### 6.1 Options and Markdown flavors

`options.rs` — all types shared between the frontend and Rust via serde:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Flavor { Commonmark, Gfm, Custom }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Paper { A4, A5, Letter, Legal }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TextAlign { Left, Center, Justified }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Margins { pub top_mm: f32, pub bottom_mm: f32, pub left_mm: f32, pub right_mm: f32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    pub flavor: Flavor,
    pub paper: Paper,
    pub margins: Margins,
    pub text_align: TextAlign,
    /// Absolute path to a user template; None = embedded default.
    pub template_path: Option<String>,
}

impl Paper {
    /// Typst `page` paper names.
    pub fn typst_name(self) -> &'static str {
        match self { Paper::A4 => "a4", Paper::A5 => "a5", Paper::Letter => "us-letter", Paper::Legal => "us-legal" }
    }
}
```

Flavor → `comrak::Options` mapping (in `markdown/mod.rs`):

| Feature | CommonMark | GFM | Custom |
|---|---|---|---|
| strikethrough | — | ✅ | ✅ |
| tables | — | ✅ | ✅ |
| autolinks | — | ✅ | ✅ |
| task lists | — | ✅ | ✅ |
| footnotes | — | — | ✅ |
| math (`$...$`) | — | — | ✅ |
| YAML front matter | — | — | ✅ |

comrak ≥ 0.50 renamed the option structs: the umbrella `Options<'c>` now has fields `extension: Extension`, `parse: Parse`, `render: Render` (previously `ExtensionOptions`/`ParseOptions`/`RenderOptions`). All of them derive `Default` and have a `bon`-based builder (default feature), so either mutate fields on a default or use `Extension::builder().table(true)…build()`. Verified against comrak 0.54.0 source (`src/parser/options.rs`):

```rust
pub fn comrak_options(flavor: Flavor) -> comrak::Options<'static> {
    let mut ext = comrak::Extension::default();
    ext.strikethrough = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.table = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.autolink = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.tasklist = matches!(flavor, Flavor::Gfm | Flavor::Custom);
    ext.footnotes = flavor == Flavor::Custom;
    ext.math_dollars = flavor == Flavor::Custom;
    ext.front_matter_delimiter = (flavor == Flavor::Custom).then(|| "---".to_owned());
    comrak::Options { extension: ext, ..comrak::Options::default() }
}
```

Notes: `front_matter_delimiter` is the only knob that turns YAML front matter parsing on (comrak 0.54, field on `Extension`); `None` (the default) leaves `---` blocks as thematic breaks / paragraphs. Do not enable `parse.smart` — smart quotes would re-type the `"` that §6.2 deliberately leaves untouched.

### 6.2 Markdown → Typst converter (`markdown/convert.rs`) — the core of the app

Signature (returns the Typst body plus any front-matter metadata so the template can render a title/author block, §6.3):

```rust
pub struct ConvertedDocument {
    pub body: String,             // Typst markup (escaped), placed inside the template's content block
    pub title: Option<String>,    // YAML front-matter `title:` value, if any
    pub author: Option<String>,   // YAML front-matter `author:` value, if any
}

pub fn md_to_typst(markdown: &str, flavor: Flavor) -> Result<ConvertedDocument, ConvertError>
```

Implementation sketch: parse with `comrak::parse_document(&arena, markdown, &opts)` and recursively walk the `NodeValue` tree (`node.data().value`, `node.children()`, `node.next_sibling()` — comrak 0.54 `Node` API). **Never call `comrak::markdown_to_html`.** Map nodes as follows. The parenthetical notes give the exact comrak 0.54 payload shapes (verified against `src/nodes.rs`):

> **comrak 0.54 node type:** `pub type Node<'a> = &'a AstNode<'a>` — `Node` **is already a reference** (it aliases `&AstNode`). Pass nodes by value everywhere (they are `Copy`); do **not** write `&Node` or `&mut Node` in signatures, and read `node.data()` (not `node.data.borrow_mut()` as in the older docs example). `AstNode<'a>` is `arena_tree::Node<'a, RefCell<Ast>>`. The codebase consistently uses `Node` (never `AstNode`) — e.g. `fn render_block(node: Node) -> String` in `convert.rs`.

| Markdown node (comrak `NodeValue`) | Typst output |
|---|---|
| `Document` | concatenation of children (blocks separated by blank lines) |
| `Heading(NodeHeading { level, .. })` | `"="` × level + space + inline content |
| `Paragraph` | inline content, followed by `\n` |
| `Text(Cow<'static, str>)` | **escaped** text (see table below) |
| `SoftBreak` | space |
| `LineBreak` | `\ ` (escaped space = hard break in Typst) |
| `Strong` | `*…*` |
| `Emph` | `_…_` |
| `Strikethrough` | `#strike[…]` |
| `Code(NodeCode { literal, .. })` | `#inline-code("…")` — string argument, so content with backticks/quotes/backslashes needs no fence logic; the prelude's `inline-code` owns the styling |
| `CodeBlock(Box<NodeCodeBlock> { info, literal, .. })` | `#code-block("lang", "source")` — `lang` is the info string's first word or `""` (the prelude owns the language fallback); the source is a string argument, so no backtick-fence math; the prelude's `code-block` owns the block chrome |
| `Link(Box<NodeLink> { url, .. })` | `#link("url")[label]` (URL-escape `"` inside the string) |
| `Image(Box<NodeLink> { url, .. })` | `#figure-image("url", [alt])` — the prelude's `figure-image` owns the default width and caption placement |
| `List(NodeList { list_type, start, .. })` | bullets stay native Typst `- ` lists (themeable via `#show list`); ordered lists become `#ordered-list(start, [a], [b], …)` — the prelude helper wraps a native Typst `enum(start:, numbering: "1.")`, so the numbering is themeable (a literal `1. ` prefix would be plain text no template could style) |
| `Item(NodeList)` | recurse children |
| `TaskItem(NodeTaskItem { symbol, .. })` | each item becomes a call to the matching prelude helper: `#task-list(task-done([…]), task-todo([…]), task-plain([…]))`. The helpers own the checkbox glyphs — pinned to the embedded DejaVu Sans (`#text(font: "DejaVu Sans")[#sym.ballot.check]` etc.) so they stay hollow and avoid the slow system-font fallback search — and the content cells; `task-list` concatenates the (glyph, content) pairs into a two-column grid. A grid is used because a native Typst list has a *single* marker for all items (§6.3) |
| `BlockQuote` | `#blockquote[…children…]` (prelude helper wrapping the native `quote(block: true)` element, Typst 0.13+) |
| `Table(Box<NodeTable>)` / `TableRow(bool)` / `TableCell` | `#markdown-table(columns: N, header: ([a], [b]), rows: (([1], [2]), …))` — Rust passes the markdown column count plus the header cells and body rows as data; the prelude's `markdown-table` emits one `auto` width per column (a bare `columns: auto` would collapse to a single column and stack the cells), bolds the header (`[*cell*]` *inside* the content block — valid in code mode, where `*[a]` would be a multiplication), and owns cell padding |
| `ThematicBreak` | `#thematic-break()` — the prelude helper owns the spacing/stroke constants (`#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)`) |
| `FootnoteReference(Box<NodeFootnoteReference>)` / `FootnoteDefinition(NodeFootnoteDefinition)` | the *reference* is inlined at its call site as `#footnote[<definition content>]` — Typst draws the superscript marker where the function is called and the note at the page bottom; the definition block itself is dropped. Definitions are collected first (label → content) and matched by `name`; comrak only recognizes `[^n]` as a reference when a definition exists, so unresolved syntax stays literal text |
| `Math(NodeMath { display_math, literal, .. })` | passthrough: inline `$…$`, block `$ … $` (LaTeX macros are **not** converted in v1; document the limitation) |
| `FrontMatter(String)` | parse YAML `title` / `author` only → `ConvertedDocument.title/author`; emit nothing in the body; everything else ignored |
| `HtmlInline(String)` / `HtmlBlock(NodeHtmlBlock { literal, .. })` | **lossy**: emit the raw literal as plain text, escaped. Never pass HTML through to Typst |

**Escaping rules for text nodes** (`escape.rs`). Typst markup reserves several characters; emit them escaped so user content cannot break out of a block or inject markup:

| Source char | Typst output |
|---|---|
| `#` | `\#` |
| `[` | `\[` |
| `]` | `\]` |
| `*` | `\*` |
| `_` | `\_` |
| `$` | `\$` |
| `\` | `\\` |
| `"` | leave as-is (template disables smart quotes, §6.3) |

Additional rules:

- **Inline code:** always `#inline-code("…")` with the string content escaped (`"` → `\"`, `\` → `\\`). Passing the code as a string argument means content containing backticks needs no special branch.
- **Code blocks:** `#code-block("lang", "source")`. The `info` string's first word is the language; if absent, Rust passes `""` and the prelude's `code-block` applies its own fallback. No highlighting is applied (see §13).
- **Line-initial `=` / `-` / `+`** inside paragraphs: these are only treated as headings/lists at block start, so they are safe inside a paragraph, but escape leading `=` after a blank-line boundary anyway if a heading-like construct is unintended — the conversion is block-aware, so this only matters if comrak's block classification differs from Typst's; the test suite (§10) covers it.
- **Empty document:** return a body of `[]` (an empty content block) so compile always succeeds.
- **Front matter:** comrak 0.54 stores the raw YAML (including the `---` delimiters stripped) in `FrontMatter(String)`. Parse only `title:` and `author:` with a plain line scan (v1 — no YAML crate; quoted/flow values are split on the first `: ` and trimmed of surrounding quotes). Everything else is ignored.

### 6.3 Template system (`typst_engine/template.rs`)

Two concepts: the **prelude** (a `.typ` file, embedded — `templates/default.typ`) and the **invocation** (the call the Rust side appends, carrying options + converted body).

**Contract — prelude + shadowing (§6.3):** the embedded file is a *prelude*: default implementations of every helper the converter emits plus a default `article`. A user template is composed **after** the prelude (`{prelude}\n\n{user template}`), so Typst shadowing applies — any function the user redefines wins, everything else falls back to the prelude default. A user template that only restyles (or defines nothing at all) is valid; the probe validation (§7.4) rejects templates that do not compile (syntax errors) or whose *overrides* are broken. The `set`/`show` rules live inside `default-article`, so a full `article` override escapes the default look; `default-article` stays reachable for overriders who want to wrap it.

The converter emits calls to these prelude functions (§6.2): `task-list`/`task-done`/`task-todo`/`task-plain`, `blockquote`, `thematic-break`, `inline-code`, `code-block`, `markdown-table`, `ordered-list`, and (via the default `article`) `title-block`. The probe body exercises all of them except `figure-image` (its image file cannot exist during validation).

**Worked example:** `examples/dark.typ` is a complete white-on-black theme that overrides `article` (page fill + light text + link/heading colors), `title-block`, the three task helpers (alternative bullets: green `✓` for done, `○` for open, `–` for plain), and the blockquote/thematic-break/inline-code/code-block/markdown-table helpers. It is covered by a regression test that compiles it through the full pipeline with zero font warnings — a reference for theme designers of which seams exist and what each one controls.

Embedded prelude (`include_str!("templates/default.typ")`):

```typ
// default.typ — embedded prelude for tauri-md-editor.
// Provides default implementations of every helper the converter emits plus
// a default `article`. User templates are composed AFTER this file, so their
// definitions shadow these defaults (§6.3).

// Task lists: each item function returns a (checkbox cell, content cell)
// pair; task-list concatenates the pairs into a two-column grid (`.pos()`
// because variadic arguments are not iterable directly). The checkbox glyphs
// are pinned to the embedded DejaVu Sans — many fonts (e.g. Ubuntu) lack the
// ballot codepoints, and letting Typst fall back across the system font book
// is ~10x slower per render AND draws a solid box instead of a hollow one.
#let task-done(body) = ([#text(font: "DejaVu Sans")[#sym.ballot.check]], [#body])
#let task-todo(body) = ([#text(font: "DejaVu Sans")[#sym.ballot]], [#body])
#let task-plain(body) = ([•], [#body])
#let task-list(..items) = {
  let cells = ()
  for item in items.pos() { cells = cells + item }
  grid(columns: (auto, 1fr), column-gutter: 8pt, row-gutter: 0.65em, ..cells)
}

// Block quotes (native Typst `quote` since 0.13).
#let blockquote(body) = quote(block: true)[#body]

// Images: default width + caption placement (overridable).
#let figure-image(url, caption) = figure(image(url, width: 80%), caption: caption)

// Thematic break: spacing/stroke constants live here, not in Rust.
#let thematic-break() = [#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)]

// Inline code (string argument avoids backtick-fence escaping).
#let inline-code(text) = raw(text)

// Code blocks: light background, monospace, no highlighting in v1.
#let code-block(lang, source) = block(
  fill: rgb("#f5f5f5"), inset: 8pt, radius: 4pt,
  raw(source, block: true, lang: if lang == "" { none } else { lang }),
)

// Markdown tables: one auto column width per markdown column (a bare
// `columns: auto` would collapse to one column and stack the cells), bold
// header (the strong markup is *inside* the content block, so it is valid in
// code mode).
#let markdown-table(columns: 1, header: (), rows: ()) = {
  let cells = ()
  for row in rows { cells = cells + row }
  let widths = ()
  for i in range(columns) { widths = widths + (auto,) }
  table(columns: widths, ..header.map(cell => [*#cell*]), ..cells.map(cell => [#cell]))
}

// Ordered lists: native enum with markdown-style "1." markers.
#let ordered-list(start, ..items) = enum(start: start, numbering: "1.", ..items)

// Title/author block from YAML front matter (§6.2) — override to restyle.
#let title-block(title, author) = {
  set align(center)
  text(size: 1.6em, weight: 700)[#title]
  if author != none {
    linebreak()
    text(size: 1.1em, weight: 500)[#author]
  }
  v(0.8em)
}

// Default fonts. Top-level (not inside the article): this runs BEFORE a user
// template, so a template's own later `#set text(font: ...)` wins, while
// templates that set no font keep DejaVu Sans (M6).
#set text(font: "DejaVu Sans", size: 11pt)

// Default article. The remaining set/show rules are function-scoped so a
// template that overrides `article` entirely escapes the default look.
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
```

**Composition** — this is the only place where Rust builds a Typst source string:

```rust
pub struct TemplateArgs<'a> {
    pub paper: &'a str,          // already the Typst name, e.g. "a4"
    pub margin_x_mm: f32,
    pub margin_y_mm: f32,
    pub text_align: &'a str,     // "left" | "center" | "justified"
    pub title: Option<&'a str>,  // front-matter title, if any (§6.2)
    pub author: Option<&'a str>, // front-matter author, if any (§6.2)
    pub body: &'a str,           // output of md_to_typst (escaped markup)
}

pub fn compose(template_source: &str, args: &TemplateArgs) -> String {
    let title = args.title.unwrap_or("none");
    let author = args.author.unwrap_or("none");
    format!(
        "{template_source}\n\n#article(\n  paper: {paper:?},\n  margin-x: {x}mm,\n  margin-y: {y}mm,\n  text-align: {align:?},\n  title: {title},\n  author: {author},\n)[\n{body}\n]",
        paper = args.paper,
        x = args.margin_x_mm,
        y = args.margin_y_mm,
        align = args.text_align,
        body = args.body,
    )
}
```

Notes:

- `{:?}` on `paper`/`text_align` yields a Rust string literal that is also a valid Typst string literal for these values (ASCII only) — a pragmatic choice; document it.
- `title`/`author` are emitted as bare identifiers: either `none` or a Rust-debug-quoted string literal (`{:?}`). Front-matter strings are user content, so a title containing `"` must be escaped with `{:?}` — reuse the same `{:?}` trick, it produces a valid Typst string literal for arbitrary ASCII content (non-ASCII passes through UTF-8 unchanged; verify with a test).
- Margins are formatted with `format!("{}", value)` — values come from a fixed enum/preset in v1, never free text.
- **The body is never interpolated into template source unescaped** — the converter already escaped it (§6.2), and it is placed *inside* a content block, which is the only place markup is valid. This mirrors the "escape on input, trust on output" rule.

**User template override:** `template_path` in `RenderOptions`. When set, read the file and compose it **after** the prelude (`{prelude}\n\n{user}`) so its definitions shadow the defaults (§6.3). If the file doesn't exist or fails a probe compile (§7.4), return a descriptive error and keep the previous template. Never fall back silently.

**Fonts:** the engine is always given the embedded DejaVu Sans (regular + bold, `include_bytes!`, permissive license). **A custom template loads only the font families it references** via `#set text(font: ...)` (`template::referenced_font_families` parses the source; `typst-kit-fonts` + `TypstKitFontOptions`), plus any font files in the template's own folder — so `#set text(font: "Times New Roman")` resolves instead of warning. Keeping the font book small matters for performance: a 700-face book makes every per-render compile ~5–7× slower (measured 378ms vs 55ms on a reference document), so subsetting to the referenced families is the default; templates that set fonts via variables (not detectable by the parser) conservatively fall back to the full system scan. The scan itself runs once per engine (re)build, i.e. once per template change, not per keystroke render. CJK and wider font coverage remain future work (§13).

### 6.4 Compile & export (`typst_engine/engine.rs`)

Uses `typst-as-lib` (v0.16.0) so implementers do **not** hand-write a `typst::World`. Two entry points, both built on one helper:

```rust
use typst_as_lib::TypstEngine;

/// Compile a complete Typst source and return the typed paged document.
/// In Typst 0.15 the document type is `PagedDocument` (typst-layout), and
/// `typst::Document` is now a *trait* implemented by export targets — add
/// `typst-layout = "0.15"` as a direct dependency to name the type.
fn compile_document(source: &str) -> Result<typst_layout::PagedDocument, EngineError> {
    let engine = TypstEngine::builder()
        .main_file(source)                          // source CONTENT, not a path
        .fonts([&*FONT_BYTES])                      // include_bytes! of one embedded font
        .build();

    // 0.16: `compile()` returns Warned<Result<Doc, TypstAsLibError>> where
    // Doc: Output; annotate the call so inference picks PagedDocument. The
    // warnings (`Warned.warnings`) must be read before `output` is consumed —
    // they are rendered alongside the doc (both error and warning diagnostics
    // are mapped to "line N: message" strings; see render_error below).
    let doc = warned.output.map_err(|e| EngineError::Compile(render_error(&e)))?;
    Ok(doc)
}
```

**Render preview pages (SVG) / export PDF (M6 — engine is built once per static source and compiled per render with `sys.inputs`):**

```rust
/// Font loading: system fonts + extra directories, in addition to the
/// embedded DejaVu Sans. Used at engine (re)build time only (§6.3).
pub struct FontOptions { pub system_fonts: bool, pub extra_dirs: Vec<PathBuf> }

/// Build the engine for a static source. Called by the command layer's cache
/// (§7.5), so the font scan runs once per source change.
pub fn build_engine(
    source: &str,
    fonts: &FontOptions,
) -> Result<TypstEngine<TypstTemplateMainFile>, EngineError> {
    let mut builder = TypstEngine::builder()
        .main_file(source)                          // source CONTENT, not a path
        .fonts([FONT_BYTES, FONT_BOLD_BYTES]);      // embedded DejaVu Sans
    if fonts.system_fonts || !fonts.extra_dirs.is_empty() {
        let mut opts = typst_kit_options::TypstKitFontOptions::default();
        if !fonts.system_fonts { opts = opts.include_system_fonts(false); }
        if !fonts.extra_dirs.is_empty() { opts = opts.include_dirs(fonts.extra_dirs.iter().cloned()); }
        builder = builder.search_fonts_with(opts);
    }
    // Keep the comemo memoization cache warm across renders (font loading and
    // shared work would otherwise be re-done on every keystroke — ~7x slower
    // with system fonts); bound memory by evicting entries unused for a while.
    builder.comemo_evict_max_age(Some(30));
    Ok(builder.build())
}

/// Compile the static source with per-render sys.inputs → per-page SVGs.
pub fn compile_preview_with_inputs(
    engine: &TypstEngine<TypstTemplateMainFile>,
    inputs: &Dict,                       // { title, author, body } (§7.4)
) -> Result<PreviewOutcome, EngineError> {
    let warned = engine.compile_with_input(inputs.clone());
    let doc: PagedDocument = warned.output.map_err(|e| EngineError::Compile(render_error(engine, &e)))?;
    let warnings = warned.warnings.iter().map(|d| render_diagnostic(engine, d)).collect();
    // typst-svg 0.15: svg(page: &Page, opts: &SvgOptions) — the whole page,
    // not `&page.frame`; the `pages` field is private, use `doc.pages()`.
    let pages = doc.pages().iter().map(|p| typst_svg::svg(p, &Default::default())).collect();
    Ok(PreviewOutcome { pages, warnings })
}

/// Compile the static source with per-render sys.inputs → PDF bytes.
pub fn compile_pdf_with_inputs(
    engine: &TypstEngine<TypstTemplateMainFile>,
    inputs: &Dict,
) -> Result<Vec<u8>, EngineError> {
    let warned = engine.compile_with_input(inputs.clone());
    let doc: PagedDocument = warned.output.map_err(|e| EngineError::Compile(render_error(engine, &e)))?;
    typst_pdf::pdf(&doc, &Default::default()).map_err(|e| EngineError::Pdf(e.to_string()))
}
```

**API-verification checklist** (mandatory before coding — these signatures drift between Typst releases). Verified facts for the 0.15/0.16 line (from docs.rs, June–July 2026), re-confirm against the exact locked version:

1. `TypstEngine::builder()` → `TypstTemplateEngineBuilder`; `.main_file(source: impl IntoSource)` takes source *content*; `.fonts([&[u8]])` takes byte slices; `.build()` → `TypstEngine<TypstTemplateMainFile>`. ✓ (docs.rs typst-as-lib 0.16.0)
2. `engine.compile_with_input(dict)` compiles the main file with `sys.inputs` set; returns `Warned<Result<Doc, TypstAsLibError>>` where `Doc: Output` (annotate as `PagedDocument`); unwrap via `.output`. `engine.compile()` is the no-inputs variant. ✓
3. `builder.search_fonts_with(TypstKitFontOptions)` (feature `typst-kit-fonts`) loads system fonts / extra dirs at engine build time; `TypstKitFontOptions::default()` has `include_system_fonts: true`; `.include_dirs([paths])` scans recursively. ✓ (typst-as-lib 0.16.0 `font_searcher` example)
4. `typst_pdf::pdf(&doc, &PdfOptions) -> Result<Vec<u8>, _>` — returns bytes; options struct is `PdfOptions` (has `Default`). ✓
5. `typst_svg::svg(page: &Page, opts: &SvgOptions) -> String` — first argument is a page `Page` (from `doc.pages`), not a frame. ✓ (docs.rs typst-svg 0.15.1)
6. `PagedDocument` lives in typst-layout 0.15 (`typst_layout::document::PagedDocument`, re-exported as `typst_layout::PagedDocument`); pages are read via the `pages()` accessor (the field is private in 0.15.1), yielding `&[Page]`. ✓ (verified against typst-layout 0.15.1 source)

**Line numbers:** `Span` has no `linenum()` in 0.15. Resolve a diagnostic's byte range with `world.range(span)` (the `WorldExt` trait in typst-library), then `world.source(span.id())?.lines().byte_to_line(range.start) + 1`. Warnings come from `Warned.warnings: EcoVec<SourceDiagnostic>` and are rendered the same way; never fatal (§9).

**Error rendering:** `TypstAsLibError` wraps the compile errors; use `engine.with_world(|world| …)` (0.16 API) or `world_builder()` to access the `TypstWorld` and map the first ~5 `SourceDiagnostic`s to `"line {span.linenum()}: {message}"`-style strings. Warnings (`Warned.warnings`) are a separate list returned to the UI.

## 7. Tauri shell

### 7.1 Bootstrap

1. Scaffold: `npm create vite@latest` → **React** (JS template) + `npm create tauri-app@latest` (or the manual files in this spec), keep the default `src/` + `src-tauri/` layout. Add Mantine 8 + CodeMirror deps (§4.2). *(Done in M0 — see §11.)*
2. `npm install` (frontend deps, §4.2).
3. Add plugins & crates (§4.1): `cargo add tauri-plugin-dialog`, etc.
4. Register the dialog plugin in `lib.rs` and add its permissions (§7.3).
5. Dev loop: `npm run tauri dev`. Production: `npm run tauri build`.

`vite.config.js` (standard Tauri template):

```js
import { defineConfig } from "vite";
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
});
```

### 7.2 Configuration (`src-tauri/tauri.conf.json`)

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "tauri-md-editor",
  "version": "0.1.0",
  "identifier": "com.example.tauri-md-editor",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [{ "title": "Markdown to PDF", "width": 1200, "height": 800, "minWidth": 800, "minHeight": 500 }],
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:"
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]
  }
}
```

CSP notes: `style-src 'unsafe-inline'` is required for CodeMirror's injected styles. No `script-src` additions — no CDNs, no inline scripts. `img-src data:` is not needed in v1 (SVG is inline DOM, not images) but is harmless and future-proof. Generate `icons/` with `npx tauri icon <source.png>`.

### 7.3 Capabilities (`src-tauri/capabilities/default.json`)

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default", "core:window:allow-set-title"]
}
```

`dialog:default` covers open/save/message dialogs; `core:window:allow-set-title` lets the frontend keep the OS window title in sync with the document title (§8.4). Do **not** add `core:webview` or any filesystem plugin permissions — file I/O happens inside Rust commands via `std::fs` (§7.4), not through frontend plugins. This is the principle of least privilege.

### 7.4 Commands (`commands.rs`)

All commands are async and follow one shape: `#[tauri::command] async fn name(app: AppHandle, payload: X) -> Result<Y, String>`. Return `Err(String)` with a user-presentable message; the frontend renders it in the error banner.

```rust
use crate::markdown::convert::md_to_typst;
use crate::options::RenderOptions;
use crate::typst_engine::template::{self, TemplateArgs};
use crate::typst_engine::engine;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOutcome { pub pages: Vec<String>, pub warnings: Vec<String>, pub seq: u64 }

/// Live preview: markdown + options → per-page SVG strings (M6: engine cache).
#[tauri::command]
pub async fn render_preview(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    payload: RenderPayload,        // { markdown, options, seq }
) -> Result<PreviewOutcome, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let static_source = template::build_static_source(&template_source, &payload.options)?;
    let inputs = template::render_inputs(&payload.markdown, &payload.options)?;
    let fonts = font_options_for(&payload.options);   // system fonts + template dir

    let cache = state.inner().clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache.lock().map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = cache.engine_for(&static_source, &fonts)?; // rebuilds on change
        engine::compile_preview_with_inputs(engine, &inputs).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Preview task failed: {e}"))??;
    Ok(PreviewOutcome { pages: outcome.pages, warnings: outcome.warnings, seq: payload.seq })
}

/// Export: markdown + options → system save dialog → write PDF to disk (M6).
#[tauri::command]
pub async fn export_pdf(
    app: AppHandle,
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    payload: RenderPayload,
    title: String,
    directory: Option<String>,
) -> Result<serde_json::Value, String> {
    let template_source = resolve_template_source(&payload.options)?;
    let static_source = template::build_static_source(&template_source, &payload.options)?;
    let inputs = template::render_inputs(&payload.markdown, &payload.options)?;
    let fonts = font_options_for(&payload.options);

    let cache = state.inner().clone();
    let pdf = tauri::async_runtime::spawn_blocking(move || {
        let mut cache = cache.lock().map_err(|_| "engine cache lock poisoned".to_string())?;
        let engine = cache.engine_for(&static_source, &fonts)?;
        engine::compile_pdf_with_inputs(engine, &inputs).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Export task failed: {e}"))??;

    let file_path = app
        .dialog()
        .file()
        .set_title("Export PDF")
        .add_filter("PDF", &["pdf"])
        .set_file_name("document.pdf")
        .blocking_save_file()       // blocking dialog is fine off the UI thread
        .map_err(|e| format!("Save dialog failed: {e}"))?;

    if let Some(path) = file_path {
        std::fs::write(&path, pdf).map_err(|e| format!("Failed to write PDF: {e}"))?;
        Ok(serde_json::json!({ "path": path.to_string_lossy() }))
    } else {
        Ok(serde_json::json!({ "canceled": true }))
    }
}

#[tauri::command]
pub async fn open_markdown(app: AppHandle) -> Result<serde_json::Value, String> {
    let picked = app.dialog().file()
        .set_title("Open Markdown")
        .add_filter("Markdown", &["md", "markdown"])
        .add_filter("All files", &["*"])
        .blocking_pick_file()
        .map_err(|e| format!("Open dialog failed: {e}"))?;
    match picked {
        Some(path) => {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read file: {e}"))?;
            Ok(serde_json::json!({ "path": path.to_string_lossy(), "content": content }))
        }
        None => Ok(serde_json::json!({ "canceled": true })),
    }
}

#[tauri::command]
pub async fn save_markdown(app: AppHandle, path: Option<String>, content: String) -> Result<serde_json::Value, String> {
    // If no path (untitled), show save dialog; then write atomically.
    let path = match path {
        Some(p) => Some(p.into()),
        None => app.dialog().file()
            .set_title("Save Markdown")
            .add_filter("Markdown", &["md"])
            .set_file_name("document.md")
            .blocking_save_file().map_err(|e| format!("Save dialog failed: {e}"))?,
    };
    if let Some(p) = path {
        // atomic-ish: write temp + rename; at minimum fs::write
        std::fs::write(&p, content).map_err(|e| format!("Failed to save: {e}"))?;
        Ok(serde_json::json!({ "path": p.to_string_lossy() }))
    } else {
        Ok(serde_json::json!({ "canceled": true }))
    }
}

/// Validate a custom template by compiling a probe document (§6.3, M6).
#[tauri::command]
pub async fn validate_template(
    state: State<'_, Arc<Mutex<TemplateEngineCache>>>,
    path: String,
    options: RenderOptions,
) -> Result<serde_json::Value, String> {
    let user_source = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read template: {e}"))?;
    let template_source = format!("{DEFAULT_TEMPLATE}\n\n{user_source}");   // composed
    let static_source = template::build_static_source(&template_source, &options)?; // after the
    let fonts = font_options_for_path(&path);                               // prelude (§6.3)
    let cache = state.inner().clone();
    spawn_blocking(move || {
        let mut cache = cache.lock().map_err(|_| "cache lock poisoned".to_string())?;
        let engine = engine::build_engine(&static_source, &fonts)?;        // scan happens here
        engine::compile_preview_with_inputs(&engine, &template::probe_inputs())
            .map_err(|e| e.to_string())?;                                  // probe-compile
        cache.prime(&static_source, engine);                               // reuse for render
        Ok::<(), String>(())
    }).await.map_err(|e| format!("Validation task failed: {e}"))?
      .map_err(|e| format!("Template at {path} rejected: {e}"))?;
    Ok(serde_json::json!({ "valid": true }))
}
```

**Implementation notes (M4, verified against tauri-plugin-dialog 2.7):** the *blocking* dialog variants return `Option<FilePath>` directly (no `Result`); call `FilePath::into_path()` to obtain a `PathBuf`. Writes use `files.rs::write_atomically` (temp sibling + rename — atomic on POSIX, replaces on Windows). Exported PDFs are then opened in the system viewer with the `open` crate — Rust-side only, so it adds no webview capability and keeps the §7.3 least-privilege posture. A dismissed dialog resolves as `{ "canceled": true }`, which the UI treats as silent (§9). The dialog-bearing commands take `directory: Option<String>` (dialogs start there via `set_directory`, skipped when empty) and `title: String` (default file name via `set_file_name("<title>.md"` / `"<title>.pdf"`)); `get_initial_directory` reads `$XDG_DATA_HOME`/`$HOME` for the startup working directory (§8.4). `open_external(url)` validates the scheme with `links::is_browser_url` (http/https only, §7.4) and forwards it to the system browser via the `open` crate — the webview never navigates to a link (§9); a `tauri::Builder`-level `on_navigation` interception is future hardening (§13). Footnote markers are internal links in the PDF export (Typst emits `DirectLinkElem`), so PDF viewers can jump marker → note and back via viewer navigation; in the SVG preview typst-svg emits no `href` for internal destinations (no link resolver is passed), so footnote markers are not clickable there — and if a resolver is ever added, fragment links already bypass the external-link gate (§9).

**`build_static_source` / `render_inputs` — shared pure logic** (kept in `template.rs` so it is unit-testable without Tauri; M6 splits the old inline-body `build_source` into the static source and the per-render inputs):

```rust
/// The static part: template + article invocation reading title/author/body
/// from sys.inputs. Rebuilt (and the engine re-created) only when the
/// template or these toolbar options change.
pub fn build_static_source(
    template_source: &str,          // already read from default or user path
    options: &RenderOptions,
) -> Result<String, String> {
    Ok(compose(template_source, &TemplateArgs {
        paper: options.paper.typst_name(),
        margin_x_mm: options.margins.left_mm.min(options.margins.right_mm),
        margin_y_mm: options.margins.top_mm.min(options.margins.bottom_mm),
        text_align: match options.text_align { TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Justified => "justified" },
    }))
}

/// The per-render sys.inputs: title/author from front matter (none or string)
/// and the converted + escaped body. Built on every render; the engine is
/// untouched by it.
pub fn render_inputs(markdown: &str, options: &RenderOptions) -> Result<Dict, String> {
    let converted = md_to_typst(markdown, options.flavor);
    let mut inputs = Dict::new();
    inputs.insert("title".into(), converted.title.into_value());
    inputs.insert("author".into(), converted.author.into_value());
    inputs.insert("body".into(), converted.body.into_value());
    Ok(inputs)
}
```

(Margins use the *smaller* of the two horizontal/vertical values for the `x`/`y` shorthand — or, cleaner, pass all four to the template and let `article` accept `top/bottom/left/right`. Pick one and keep the template and `TemplateArgs` in sync; the test suite must pin the chosen behaviour.)

### 7.5 Async & concurrency notes

- Typst compilation is CPU-bound and can take tens of milliseconds on large documents. Run it in `tauri::async_runtime::spawn_blocking` (Tokio's `spawn_blocking`) so the IPC handler doesn't stall the runtime. Readers comfortable with Tokio will recognize this as the standard pattern for CPU work in an async runtime.
- **Stale responses:** rapid typing can interleave responses. The frontend includes a monotonically increasing `seq` in each `render_preview` payload; the command echoes it back; the frontend drops any response whose `seq` is older than the last request it sent (§8.3). This is the cheapest correct solution; a real cancellation mechanism is future work.
- **Engine cache (M6):** one `TypstEngine` per *static source* lives in `tauri::State<Arc<Mutex<TemplateEngineCache>>>`. The static source is the resolved template (prelude + user, or default) plus the toolbar options — it never contains the markdown body, which travels as `sys.inputs` per render. The command resolves the static source, builds the inputs, then `spawn_blocking`s a closure that locks the cache, rebuilds the engine only when the source changed (re-running the font scan), and compiles. Compiles are therefore serialized through the mutex — fine for a single-user editor; concurrency/parallelism is future work.
- **Per-render inputs:** `template::render_inputs(markdown, options)` converts the markdown and returns the `sys.inputs` dictionary (`title`, `author`, `body`). The static source's invocation reads them and `#eval`s the body as markup with an explicit `scope:` of every converter helper (§6.3) — `eval` runs in a fresh scope that otherwise only sees the standard library.
- **Memoization across renders:** `build_engine` sets `comemo_evict_max_age(Some(30))` instead of typst-as-lib's default `Some(0)`. The default wipes the comemo cache after every compile, re-reading font files and re-doing shared work on each keystroke render (measured ~7× slower with system fonts: ~190ms → ~27ms per render on a reference document). A bounded eviction age keeps font loading and recent compiles warm while bounding memory; the cache keys on `sys.inputs`, so distinct bodies never serve stale previews (pinned by a test).
- **Template switches don't double-scan (M6):** `validate_template` receives the current options, builds the real engine (paying the font scan once), probe-compiles it, and `prime`s the engine cache. The subsequent `render_preview` for the same static source finds the engine ready — so the document re-renders immediately instead of scanning a second time.

### 7.6 `lib.rs` registration

```rust
mod commands; mod markdown; mod options; mod typst_engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(std::sync::Arc::new(std::sync::Mutex::new(
            commands::TemplateEngineCache::new(),
        ))) // M6 engine cache (§7.5)
        .invoke_handler(tauri::generate_handler![
            commands::render_preview,
            commands::open_markdown,
            commands::save_markdown,
            commands::export_pdf,
            commands::get_initial_directory, // §8.4
            commands::open_external,         // §7.4
            commands::validate_template,     // M5
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`main.rs`:

```rust
fn main() { tauri_md_editor_lib::run() }
```

## 8. Frontend

### 8.1 Component tree (`src/`)

React 19 + Mantine 8. Components own their markup and styles; `index.html` stays a bare shell (root div + module script). File structure (§5):

```
src/
├── main.jsx          # entry: font imports, Mantine theme, <MantineProvider>, <App/>
├── App.jsx           # app state, debounce loop, seq guard, layout
├── components/
│   ├── Toolbar.jsx   # file buttons, flavor/paper/margins/align selects, template, export
│   ├── Editor.jsx    # CodeMirror 6 via @uiw/react-codemirror
│   ├── Preview.jsx   # SVG pages from render_preview
│   ├── StatusBar.jsx # page count, warnings, save state
│   └── ErrorBanner.jsx
├── lib/
│   ├── api.js        # invoke() wrappers (one per command)
│   ├── settings.js   # localStorage-backed settings
│   └── presets.js    # FLAVORS, PAPERS, MARGIN_PRESETS, ALIGNS tables
└── styles.css        # app-level layout only; Mantine styles components
```

`index.html` (Vite React template, trimmed):

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Markdown to PDF</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.jsx"></script>
  </body>
</html>
```

`main.jsx` — fonts + theme first, then render:

```jsx
import "@fontsource/iosevka/400.css";
import "@fontsource/iosevka/500.css";
import "@fontsource-variable/archivo-narrow/wght.css";
import { MantineProvider, createTheme } from "@mantine/core";
import App from "./App.jsx";

const theme = createTheme({
  fontFamily: "Archivo Narrow Variable, sans-serif",
  fontSizes: { xs: "13px", sm: "15px", md: "17px", lg: "19px", xl: "21px" },
});

createRoot(document.getElementById("root")).render(
  <MantineProvider theme={theme}>
    <App />
  </MantineProvider>,
);
```

Layout stays a plain flexbox skeleton (Mantine handles the individual controls):

```jsx
<div className="app">            {/* flex column, 100vh */}
  <Toolbar ... />
  <main id="panes">              {/* flex row, flex: 1 */}
    <section id="editor-pane"><Editor ... /></section>
    <section id="preview-pane"><Preview pages={pages} /></section>
  </main>
  <StatusBar ... />
  <ErrorBanner ... />
</div>
```

### 8.2 Editor (`components/Editor.jsx`)

CodeMirror 6 via `@uiw/react-codemirror` (the maintained React wrapper). It is a controlled component: value/onChange come from `App`. A save shortcut triggers the toolbar's save handler:

```jsx
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

export default function Editor({ value, onChange, onSave }) {
  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      className="editor"
      extensions={[
        history(),
        markdown(),
        keymap.of([
          ...defaultKeymap,
          ...historyKeymap,
          { key: "Mod-s", run: () => { onSave(); return true; } },
        ]),
      ]}
      basicSetup={{ foldGutter: false }}
    />
  );
}
```

Notes:

- `@uiw/react-codemirror` wraps `EditorView` in a React lifecycle; `onChange` fires with the doc string on every edit. No manual `EditorState`/`EditorView` construction, no `parent` element juggling.
- Keep `@codemirror/lang-markdown` and `@codemirror/commands` as explicit deps (not just transitive) — they are pinned in §4.2.
- Styling: the wrapper receives `.editor` (Iosevka 16px, see §8.5); Mantine resets are neutralized by CodeMirror's own theme. `basicSetup` keeps line numbers, search, etc.; `foldGutter` off keeps the gutter narrow in v1.

### 8.3 Preview (`components/Preview.jsx`)

Renders the SVG pages returned by `render_preview` as inline DOM (safe: our compiler produced them). The stale-response guard lives in `App` (it compares against the sequence it last sent — §7.5); the component is pure:

```jsx
export default function Preview({ pages }) {
  return (
    <div id="preview">
      {pages.map((svg, i) => (
        <div
          key={i}
          className="page"
          data-page={i + 1}
          dangerouslySetInnerHTML={{ __html: svg }}
        />
      ))}
    </div>
  );
}
```

`dangerouslySetInnerHTML` is acceptable here because the strings come from `render_preview` (our Typst compile), never from user input directly — the same trust boundary as the old `innerHTML` assignment. The pages update wholesale on every compile. Incremental / patch-based updates (typst-preview style) are future work (§13).

### 8.4 `App.jsx` state and `lib/api.js`

`api.js` — one wrapper per command (using `@tauri-apps/api` v2):

```js
import { invoke } from "@tauri-apps/api/core";

export const renderPreview = (payload) => invoke("render_preview", { payload });
export const exportPdf = (payload) => invoke("export_pdf", { payload });
export const openMarkdown = () => invoke("open_markdown");
export const saveMarkdown = (path, content) => invoke("save_markdown", { path, content });
export const validateTemplate = (path) => invoke("validate_template", { path });
```

`App.jsx` — owns the app state, the debounce loop and the seq guard. `useDebouncedCallback` comes from `@mantine/hooks`; settings are loaded once from `lib/settings.js` (§8.6):

```jsx
import { useCallback, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import * as api from "./lib/api.js";
import { loadSettings, saveSettings } from "./lib/settings.js";

export default function App() {
  const [settings] = useState(() => loadSettings());
  const [markdown, setMarkdown] = useState(INITIAL_DOC);
  const [options, setOptions] = useState(settings.options);   // { flavor, paper, margins, textAlign, templatePath }
  const [pages, setPages] = useState([]);
  const [warnings, setWarnings] = useState([]);
  const [error, setError] = useState(null);
  const filePath = useRef(null);
  const seq = useRef(0);                // last seq we *sent*

  const refreshPreview = useDebouncedCallback(async () => {
    const seqId = ++seq.current;
    try {
      const result = await api.renderPreview({ markdown, options, seq: seqId });
      if (result.seq < seq.current) return;   // stale — a newer request is in flight
      setPages(result.pages);
      setWarnings(result.warnings);
      setError(null);
    } catch (err) {
      setError(String(err));            // keep last good preview (pages untouched)
    }
  }, 200);

  const handleEditorChange = useCallback((value) => {
    setMarkdown(value);
    refreshPreview();
  }, [refreshPreview]);

  // settings persistence on option change
  const changeOption = (patch) => {
    const next = { ...options, ...patch };
    setOptions(next);
    saveSettings({ ...settings, options: next });
    refreshPreview();
  };

  return (
    <div className="app">
      <Toolbar options={options} onOptionChange={changeOption}
               onExport={handleExport} onOpen={handleOpen} onSave={handleSave} />
      <main id="panes">
        <section id="editor-pane">
          <Editor value={markdown} onChange={handleEditorChange} onSave={handleSave} />
        </section>
        <section id="preview-pane"><Preview pages={pages} /></section>
      </main>
      <StatusBar pages={pages.length} warnings={warnings} />
      <ErrorBanner message={error} onClose={() => setError(null)} />
    </div>
  );
}
```

`Toolbar.jsx` — Mantine controls mapped to `options`; `Select` data comes from `lib/presets.js`:

```jsx
import { Button, Divider, Group, Select } from "@mantine/core";
import { IconFileTypePdf, IconFolderOpen, IconFilePlus, IconTemplate } from "@tabler/icons-react";
import { FLAVORS, PAPERS, MARGIN_PRESETS, ALIGNS } from "../lib/presets.js";

export default function Toolbar({ options, onOptionChange, onExport, onOpen, onSave, onNew }) {
  return (
    <header id="toolbar">
      <Button variant="subtle" leftSection={<IconFilePlus size={16} />} onClick={onNew}>New</Button>
      <Button variant="subtle" leftSection={<IconFolderOpen size={16} />} onClick={onOpen}>Open</Button>
      <Button variant="subtle" leftSection={<IconFileTypePdf size={16} />} onClick={onSave}>Save</Button>
      <Divider orientation="vertical" />
      <Select label="Flavor" data={FLAVORS} value={options.flavor} onChange={(v) => onOptionChange({ flavor: v })} w={140} />
      <Select label="Paper" data={PAPERS} value={options.paper} onChange={(v) => onOptionChange({ paper: v })} w={110} />
      <Select label="Margins" data={MARGIN_PRESETS} value={options.marginPreset} onChange={(v) => onOptionChange({ marginPreset: v })} w={170} />
      <Select label="Align" data={ALIGNS} value={options.textAlign} onChange={(v) => onOptionChange({ textAlign: v })} w={110} />
      <div className="spacer" />
      <Button variant="default" leftSection={<IconTemplate size={16} />} onClick={onPickTemplate}>Template…</Button>
      <Button leftSection={<IconFileTypePdf size={16} />} onClick={onExport}>Export PDF</Button>
    </header>
  );
}
```

File handlers stay thin: `onOpen` → `api.openMarkdown()` → `setMarkdown(res.content)`, `filePath.current = res.path`; `onSave` → `api.saveMarkdown(filePath.current, markdown)` and update the ref; `onExport` → `api.exportPdf({ markdown, options })`; `onPickTemplate` → `tauri-plugin-dialog`'s `open()` from the frontend (filter `.typ`), then `api.validateTemplate(path)` and only on success `onOptionChange({ templatePath: path })`. `canceled: true` results are silent (§9).

**Title & working directory (M4, §8.4).** The app always knows the current document's `title` and a `directory` it works in. Title derivation (`lib/title.js`): front-matter `title` wins (parsed with the same rules as the Rust `parse_front_matter` — split on `": "`, trim, strip one pair of double quotes), else the opened file's stem, else `Sample` for the untitled sample document. The directory starts as `$XDG_DATA_HOME` (else `$HOME`, via `get_initial_directory`) and follows Open: the opened file's parent directory becomes the new Directory. Save is always a "Save As…" dialog defaulting to `<title>.md` in the Directory; Export defaults to `<title>.pdf` there. Titles are sanitized before use as file names (`safeFileName`: strips `/`, `\`, reserved characters and control chars, collapses whitespace, falls back to `document`). The OS window title is kept in sync as `<title> - Markdown to PDF` (`windowTitle`, granted via `core:window:allow-set-title`, §7.3) so multiple instances are distinguishable in the window manager. Title derivation (frontmatter parsing) and the window-title IPC update ride the same 200 ms debounce as the preview (§7.5): typing in the frontmatter never parses the document or pushes a window-title update per keystroke — `title` is state refreshed by a debounced callback.

### 8.5 Styles (`styles.css`)

Mantine styles the controls; the stylesheet covers only the app skeleton and the preview pages. Same flexbox fundamentals as before: `display:flex` on the panes row, `flex:1` on each pane.

```css
* { box-sizing: border-box; }
html, body, #root { height: 100%; margin: 0; }

.app { display: flex; flex-direction: column; height: 100%; }

#toolbar { display: flex; align-items: flex-end; gap: 12px; padding: 8px 12px;
           border-bottom: 1px solid var(--mantine-color-gray-3); }
#toolbar .spacer { flex: 1; }

#panes { flex: 1; display: flex; min-height: 0; }
#editor-pane  { flex: 1; min-width: 0; display: flex; flex-direction: column; }
#preview-pane { flex: 1; min-width: 0; background: #525659; overflow: auto; }

.editor { flex: 1; min-height: 0; overflow: hidden; }
.editor .cm-editor { height: 100%; font-family: Iosevka, monospace; font-size: 16px; }

#preview { padding: 24px; }
/* Each SVG page is a white sheet with a shadow, centered, sized to the pane width. */
.page { background: #fff; box-shadow: 0 2px 8px rgba(0,0,0,.35);
        width: 100%; max-width: 820px; margin: 0 auto 24px; }
.page svg { display: block; width: 100%; height: auto; }
```

Key detail for the preview: Typst emits SVGs with an intrinsic pixel size (e.g. A4 @ 96dpi ≈ 794×1123). `width:100%; height:auto` scales them to the pane while preserving aspect ratio, so pages behave like a real document viewer.

### 8.6 Settings (`lib/settings.js`)

v1 persists to `localStorage` under one JSON key (e.g. `tauri-md-editor.settings`): `{ options: { flavor, paper, marginPreset, margins, textAlign, templatePath }, editorFontSize }`. Provide a preset table for margins (Normal / Narrow / Custom) in `lib/presets.js`:

```js
export const FLAVORS = [
  { value: "commonmark", label: "CommonMark" },
  { value: "gfm",        label: "GFM" },
  { value: "custom",     label: "Custom" },
];
export const PAPERS = [
  { value: "a4", label: "A4" }, { value: "a5", label: "A5" },
  { value: "letter", label: "Letter" }, { value: "legal", label: "Legal" },
];
export const MARGIN_PRESETS = [
  { value: "normal", label: "Normal (20/25mm)" },
  { value: "narrow", label: "Narrow (10/15mm)" },
  { value: "custom", label: "Custom…" },
];
export const ALIGNS = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "justified", label: "Justified" },
];
```

`loadSettings()`/`saveSettings()` are pure `JSON.parse`/`JSON.stringify` around the key, with a `try/catch` fallback to defaults on corrupt data. `tauri-plugin-store` (a file-backed store) is an acceptable alternative but not required in v1.

## 9. Error handling & UX states

- Every command returns `Err(String)` with a message that starts with a human actionable phrase ("Failed to write PDF:", "Preview failed:", …). The frontend shows it in the error banner and keeps the last good preview.
- **Empty document:** the converter returns `[]` (§6.2), so preview shows a single blank page. Show a hint in the status bar ("Nothing to render" is not needed — a blank page is acceptable and simpler).
- **Compile errors** (e.g. broken custom template, unsupported construct): show the first ~5 diagnostics in the banner; keep the previous preview. Since M6, the markdown body is `eval`'d from `sys.inputs` (§7.5), so a diagnostic inside the body points into the eval'd string rather than the composed source; the message text (e.g. "unknown variable: …") is what matters — the line number may be relative to the body.
- **Warnings** (missing font, etc.): shown in the status bar as `N warning(s)` and in a collapsible list; never fail the render.
- **Missing/custom template:** errors on read or probe compile are surfaced with the offending path.
- **Export cancel:** the dialog returning `None` is *not* an error (the JSON includes `canceled: true`); the UI must not show a banner for it.
- **External links:** the preview SVG contains real `<a>` links (Typst emits them, §6.4). The frontend intercepts every click (`Preview.jsx` `preventDefault` on `closest("a")`) and calls `open_external(url)`: http(s) opens in the system default browser, any other scheme is rejected with a banner error ("Blocked external link: …"). Same-document fragment links (`href` starting with `#`) are internal (e.g. PDF-style footnote/label jumps) and are left to the webview to scroll natively — they are never forwarded to the browser. The webview must never navigate away from the app.

## 10. Testing (TDD)

Follow red-green-refactor. Because `markdown/` and `typst_engine/` are plain Rust (no `tauri` imports), tests run with plain `cargo test` — no app startup needed. Suggested suite:

**`markdown/convert.rs` tests (table-driven — the bulk of the value):**

- `escapes hash/brackets/asterisks/underscores/dollar/backslash in text`
- `maps heading levels to =, ==, …`
- `maps bold/italic/strikethrough`
- `maps inline code and code blocks (with language, without language, containing backticks)`
- `maps links with and without label, URL containing quotes`
- `maps ordered and unordered lists, nested lists`
- `maps task list items to #task-list(task-done([…]), task-todo([…]), …)` helper calls`
- `maps GFM table to #markdown-table(header: …, rows: …)` (only when flavor = GFM/Custom; in CommonMark comrak yields paragraphs)
- `maps footnotes and math in Custom flavor; leaves them as text otherwise`
- `strips HTML tags and keeps text` (lossy path)
- `parses YAML front matter title/author; ignores other keys` (no title/author → `ConvertedDocument` with `None`; title containing quotes → valid escaped Typst string)
- `empty input returns []`

**`typst_engine/template.rs` tests:**

- `compose emits template + article invocation with expected args` (static source: toolbar options + `sys.inputs` reads + helper `scope:`)
- `build_static_source maps paper/margins/alignment to template args`
- `render_inputs forwards front-matter title/author and the converted body; emits none without front matter`
- `prelude contract: default template compiles a probe document; user templates are composed after the prelude (resolve_template_source prepends it)`
- `probe_template: accepts a styling-only template (defaults apply), accepts valid helper overrides, rejects syntax errors and broken overrides`

**Frontend (optional in M1, required later):** the pure modules (`lib/presets.js`, `lib/settings.js`) are testable with Vitest. Component tests (React Testing Library) are not required for v1; the seq guard and debounce are simple enough to review by hand — keep them in `App.jsx` so they are auditable.

**`typst_engine/engine.rs` tests (slow but few — mark with `#[ignore]` or keep tiny):**

- `compile_preview returns at least one SVG containing "<svg"`
- `compile_pdf bytes start with "%PDF" and are non-empty`
- `compile error path yields diagnostics, not a panic`
- `custom template with a broken override fails the probe compile`
- `warnings are collected not fatal` (unknown font → warning, not error)
- `system fonts are used when enabled` (a real installed family renders warning-free with `FontOptions::system_fonts`, warns without; skips on font-less machines)

**Command-layer tests (optional, Tauri 2):** `tauri::test::mock_builder` / `mock_app` can invoke commands in-process; at minimum assert that `build_static_source`/`render_inputs` (shared logic) are covered by the unit tests above, and that the `TemplateEngineCache` serves/rebuilds per static source. Full IPC testing is not required for v1.

Example test style:

```rust
#[test]
fn escapes_special_characters_in_text() {
    let cases = [
        ("a # b", "a \\# b"),
        ("x [y] z", "x \\[y\\] z"),
        ("*bold* not", "\\*bold\\* not"),
    ];
    for (input, expected) in cases {
        assert_eq!(convert_inline_text(input), expected, "input: {input:?}");
    }
}
```

## 11. Milestones & acceptance criteria

| Milestone | Deliverable | Acceptance |
|---|---|---|
| **M0 — Scaffold** ✅ | Project skeleton (Vite + React 19 + Tauri + plugins) | **Done** — commit `c7030ec`: `npm run tauri dev` opens an empty Mantine window; `cargo test` runs (0 tests). |
| **M1 — Converter** ✅ | `markdown/convert.rs` + `escape.rs` (TDD) | **Done** — commit `556911b`: 27 unit tests green; CommonMark/GFM/Custom flavour differences verified by tests. |
| **M2 — Engine** ✅ | `typst_engine/` — template, compose, compile | **Done** — `compile_preview(sample)` returns ≥1 SVG containing `<svg`; `compile_pdf` produces bytes starting with `%PDF`; error path yields `line N: …` diagnostics; warnings are collected, not fatal. |
| **M3 — UI loop** ✅ | Editor + preview + debounce + seq guard | **Done** — CodeMirror 6 editor, Typst SVG preview, 200 ms debounce + seq guard in `App.jsx`; `render_preview` Tauri command registered; Vitest suite for `lib/presets.js` + `lib/seq.js` (14 frontend + 45 Rust tests green via `./check.sh`). |
| **M4 — Files & export** ✅ | Open/Save/Export commands wired to toolbar | **Done** — `open_markdown`/`save_markdown`/`export_pdf`/`get_initial_directory`/`open_external` registered in `lib.rs`; writes are atomic (`files.rs::write_atomically`); dialogs via `tauri-plugin-dialog` start in the working Directory and default to `<title>.md`/`<title>.pdf` (title from frontmatter → file stem → "Sample", §8.4); exported PDF opens in the system viewer via the `open` crate (Rust-side only, no new capability); task lists render as a checkbox grid and footnote references are inlined at their call site (§6.2); preview links go to the system browser, other schemes are rejected, fragment links stay internal (§9); canceled dialogs are silent; 66 Rust + 47 frontend tests green via `./check.sh`. |
| **M5 — Settings & polish** ✅ | localStorage settings, template override + validation, warnings UI, error banner | **Done** — settings persist to `localStorage` (`lib/settings.js`, loaded on startup, saved on every option change, merged over defaults on corrupt/partial data); the Template… button picks a `.typ` file via the dialog plugin and applies it only after probe validation (`validate_template` command, `template::probe_template` — the probe is composed after the prelude and exercises every converter helper plus the title/author path, §6.3); **the template system is now a prelude**: `default.typ` provides default implementations of all converter helpers (`task-done`/`task-todo`/`task-plain`, `blockquote`, `thematic-break`, `inline-code`, `code-block`, `markdown-table`, `ordered-list`, `title-block`) plus `default-article`, and user templates are composed after it so Typst shadowing gives "default unless redefined" — styling title/author means overriding `title-block`, task glyphs via the task helpers, etc.; an active custom template is shown in the toolbar; warnings render as a collapsible list in the status bar; empty doc renders exactly one blank page; 73 Rust + 54 frontend tests green via `./check.sh`. |
| **M6 — Engine cache & fonts** ✅ | Reusable engine + system/template-side fonts | **Done** — the pipeline is split into a **static source** (template + toolbar options + an `#article(...)` invocation reading `title`/`author`/`body` from `sys.inputs`, eval'd as markup with an explicit helper scope) and **per-render inputs** (`template::render_inputs`); a `TemplateEngineCache` (`tauri::State`, §7.5) holds one engine per static source and rebuilds it — re-running the font scan — only when the template or toolbar options change; custom templates load **system fonts** (`typst-kit-fonts` feature) plus any font files next to the `.typ` file (§6.3); `figure-image` added to the prelude (was missing); 77 Rust + 54 frontend tests green via `./check.sh`. |

## 12. Security

- Keep the webview defaults: no Node integration, isolation enabled.
- Capabilities: only `core:default` + `dialog:default` (§7.3). No filesystem plugin exposure; file access is Rust-only and path-gated by dialogs.
- CSP as configured in §7.2; no inline scripts, no remote resources.
- **Template execution:** a custom `.typ` template is executable Typst code from the user's perspective, but Typst has no system-call surface: its file/image access goes through the `World` implementation (typst-as-lib), which v1 restricts to the main source. Do not add a file resolver that reads arbitrary paths. Treat untrusted templates as untrusted input — validate with a probe compile before applying (§7.4), and never persist a template you could not compile.
- Remote images / package fetching (`#import "@preview/..."`) are disabled by not configuring a package/network path in the world; document that limitation rather than silently allowing network access.

## 13. Known limitations & future work

v1 limitations (accept and document in the README):

- Raw HTML in Markdown is stripped to text.
- No syntax highlighting inside code blocks in the PDF (Typst core has none built in; a `syntect`-or-`@preview/rich-codes` based highlighter can be layered on later).
- Only one embedded font by default (DejaVu Sans, regular + bold); no CJK coverage out of the box. Custom templates can reference **system fonts** and ship font files next to the `.typ` file (M6, §6.3); this is gated behind the engine cache so the scan runs once per template, not per keystroke.
- **System-font naming is typst's, not fontconfig's** — typst strips styling suffixes from family names (Condensed/Narrow/Light/…, §6.3), so a font whose *name* carries the styling is registered under the trimmed family and may be unreachable: "Ubuntu Condensed" loads but is registered as a duplicate regular "Ubuntu" face (its `usWidthClass` is normal), so `#set text(font: "Ubuntu Condensed")` warns and falls back. Use the trimmed family (`font: "Ubuntu"`), a font whose name has no styling suffix (e.g. Oswald), or fake the width with `#set text(scale: …)`. The system-font scan is a one-time cost per template switch (seconds on font-heavy machines; the scan result is cached per process), and **a large font book makes per-render compiles slow** — custom templates therefore load only the families they reference (§6.3); glyphs the selected font lacks still trigger a fallback search, so the prelude's task checkboxes are pinned to DejaVu Sans to avoid the most common case.
- Remote images unsupported; images must be local files reachable through the world's file resolver.
- No LaTeX math macros (`tex2typst-rs` could be added, as `mdxport` does).
- No incremental compilation — full recompile per change; large (50+ page) documents may show latency.
- **Linux UI text is grayscale-antialiased** — WebKitGTK (the Linux webview) renders text onto composited transparent layers, so subpixel/LCD antialiasing is impossible; `-webkit-font-smoothing` is a no-op there (Safari-macOS only). Fontconfig hinting still applies. Mitigations are font weight + size (Iosevka 500 in the editor, larger UI scale). The Typst SVG preview is vector paths and unaffected by this.
- **Non-UTF-8 filenames on legacy-locale Linux** — paths travel over IPC as JSON strings (UTF-8), so a filename in e.g. latin-2 bytes is echoed back lossily (`to_string_lossy` → U+FFFD). File *contents* still load and writes still go to the correct file (the dialog hands over the raw `PathBuf`, §7.4); the one harmful case is Save-after-Open on such a system, which writes a new mangled-named file instead of overwriting. Not an issue on UTF-8-locale Linux (the default for decades), macOS (the filesystem enforces UTF-8 names), or Windows (UTF-16 paths). A byte-safe path transport (opaque `file://` URLs or base64 over IPC) is future work (§7.4).
- SVG preview text is not selectable (inline SVG). A canvas/DOM renderer via `typst.ts` would restore selection but introduces WASM + version-pinning concerns (see §2).

Future work, in order of leverage:

1. **Structured tracing instead of `eprintln!` diagnostics** — `commands.rs` currently prints `[engine cache] rebuild: …` (`TemplateEngineCache::engine_for`) and `[preview] setup=… render=…` (`render_preview`) straight to stderr. Replace with the `tracing` crate + `tracing-subscriber` (init once in `lib.rs::run()` with an `EnvFilter` reading `RUST_LOG`), so per-render timing and cache-rebuild events are leveled (`debug!`) and opt-in rather than unconditional stderr noise.
2. **Incremental preview** — adopt `tinymist`/`typst-preview`-style patched SVG deltas (comemo memoisation) or stream pages via Tauri events instead of one big return value.
3. **Parallel/async compiles** — the M6 engine cache serializes compiles through its mutex; lifting that (per-source engine clones, or a small worker pool) would let preview and export overlap.
4. **Syntax highlighting** in code blocks (syntect pre-wrapping or Typst packages) — the prelude's `code-block` (§6.3) is the single seam to swap in a highlighter for every document.
5. **HTML export** via `typst-html` (the `mdcast` project demonstrates this).
6. **Table of contents, page numbers, running headers** — template-driven, zero core changes.
7. **`Export .typ`** button (show the composed source) — cheap, great for debugging and power users.
8. **SyncTeX-like preview↔source jumping** (click page → cursor).

**Deferred issues (noted, not yet fixed):**

- **Alignment control has no effect** (§8.4) — left/center/justified render identically (verified: the three produce byte-identical SVGs). Root cause: in `default-article` (`default.typ`) the `set align(…)` / `set par(justify: …)` rules sit *inside* the `if text-align == …` block, and Typst scopes a `set` rule to the (empty) conditional block, so it never reaches `body`. Fix: make the set unconditional with a conditional value — `set align(if text-align == "center" { center } else { left })` and `set par(justify: text-align == "justified")` — and pin it with a test that left/center/justified produce different SVGs.
- **Re-selecting the current Flavor in the toolbar deselects it** — Mantine `Select` fires `onChange(null)` when the already-selected option is clicked again (`allowDeselect` defaults to true), and `changeOption` applies the patch blindly, so `flavor: null` corrupts the settings and the IPC payload (the render then fails to deserialize). Fix: ignore `null` patches in `changeOption` (or set `allowDeselect={false}` on the toolbar Selects); the same guard applies to every single-select control in the toolbar.

## 14. References

- Tauri 2: configuration (`https://v2.tauri.app/reference/config/`), capabilities (`https://v2.tauri.app/security/capabilities/`), command docs (`https://v2.tauri.app/develop/calling-rust/`), dialog plugin (`https://v2.tauri.app/plugin/dialog/`), testing (`https://docs.rs/tauri/latest/tauri/test/`).
- Typst 0.15: `typst` (`https://docs.rs/typst`), `typst-pdf`, `typst-svg`, `typst-as-lib` (`https://docs.rs/typst-as-lib` — README has the `TypstEngine` example used in §6.4).
- Typst markup reference: `https://typst.app/docs/reference/` (text, raw, link, image, table, blockquote, strike, footnote, set rules).
- comrak: `https://docs.rs/comrak` (AST node reference for §6.2).
- Reference projects worth reading: `mdxport` (comrak → typst → PDF, in-process), `mdcast` (typst-as-lib integration, typst-html), `typst-preview` (incremental SVG preview architecture).
