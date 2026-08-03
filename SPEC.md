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
│  typst_engine::template   compose default/user template + invocation     │
│  typst_engine::engine     TypstEngine (typst-as-lib) → compile           │
│      ├── typst_svg  → per-page SVG strings  (preview)                    │
│      └── typst_pdf  → PDF bytes             (export)                     │
└──────────────────────────────────────────────────────────────────────────┘
```

**Design rule (non-negotiable):** `markdown/` and `typst_engine/` must not import `tauri`. The `#[tauri::command]` functions are thin, serialization-only wrappers. This is what makes the core unit-testable without a running app.

**Commands are stateless in v1** — each invoke compiles from scratch. Perf is acceptable (Typst compiles a typical page in single-digit milliseconds; the full document in tens of ms). Reuse of the engine / incremental compilation is future work (§13).

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
    "@fontsource/pt-sans-narrow": "^5",
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
│       └── typst_engine/
│           ├── mod.rs
│           ├── template.rs      # default.typ embedding, compose(), build_source()
│           └── engine.rs        # TypstEngine build, compile → SVG pages / PDF
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
| `Code(NodeCode { literal, .. })` | backtick raw: `` `code` `` (see backtick rule below) |
| `CodeBlock(Box<NodeCodeBlock> { info, literal, .. })` | fenced raw block: ``` `````lang````` ⏎ code ⏎ ````` ``` |
| `Link(Box<NodeLink> { url, .. })` | `#link("url")[label]` (URL-escape `"` inside the string) |
| `Image(Box<NodeLink> { url, .. })` | `#figure(image("url", width: 80%), caption: [ …optional alt text… ])` |
| `List(NodeList { list_type, start, .. })` | `- ` / `1. ` per item (list type via `NodeList.list_type: ListType::Bullet|Ordered`, ordinal via `start`) |
| `Item(NodeList)` | recurse children |
| `TaskItem(NodeTaskItem { symbol, .. })` | `#task(true/false)` + item text; checked ⇔ `symbol.is_some()` (template defines `#task`, §6.3) |
| `BlockQuote` | `#blockquote[…children…]` (native since Typst 0.13; else fallback `#quote[block(…)]`) |
| `Table(Box<NodeTable>)` / `TableRow(bool)` / `TableCell` | `#table(columns: (auto, …), [cell], [cell], …)`; `TableRow(true)` = header row, cells wrapped in `*…*` |
| `ThematicBreak` | `#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)` |
| `FootnoteReference(Box<NodeFootnoteReference>)` / `FootnoteDefinition(NodeFootnoteDefinition)` | `#footnote[…]` |
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

- **Inline code with backticks:** if the code contains `` ` ``, emit `#raw("…")` with the string content escaped (`"` → `\"`, `\` → `\\`); otherwise use `` `…` ``.
- **Code blocks:** use the fenced form ````` ```lang ````` … ````` ```. The `info` string's first word is the language; if absent, use ```` ```text ````. No highlighting is applied (see §13).
- **Line-initial `=` / `-` / `+`** inside paragraphs: these are only treated as headings/lists at block start, so they are safe inside a paragraph, but escape leading `=` after a blank-line boundary anyway if a heading-like construct is unintended — the conversion is block-aware, so this only matters if comrak's block classification differs from Typst's; the test suite (§10) covers it.
- **Empty document:** return a body of `[]` (an empty content block) so compile always succeeds.
- **Front matter:** comrak 0.54 stores the raw YAML (including the `---` delimiters stripped) in `FrontMatter(String)`. Parse only `title:` and `author:` with a plain line scan (v1 — no YAML crate; quoted/flow values are split on the first `: ` and trimmed of surrounding quotes). Everything else is ignored.

### 6.3 Template system (`typst_engine/template.rs`)

Two concepts: the **template source** (a `.typ` file, embedded by default) and the **invocation** (the call the Rust side appends, carrying options + converted body).

**Contract:** the template must define a function `article` with the following parameters. The Rust side appends an invocation of it.

Embedded default template (`include_str!("templates/default.typ")`):

```typ
// default.typ — embedded default template for tauri-md-editor.
// Contract: must define #let article(...) with these exact parameter names.
// Users may override this file; a custom template is validated by compiling
// a probe document before being accepted (§7.4).

#set page(paper: "a4", margin: (x: 20mm, y: 25mm))
#set text(font: "DejaVu Sans", size: 11pt, quote: false)
#set par(justify: false)
#set heading(weight: 600)

// Code blocks: light background, monospace, no highlighting in v1.
#show raw.where(block: true): it => block(
  fill: rgb("#f5f5f5"), inset: 8pt, radius: 4pt, it,
)

// Links: styled like the classic markdown look.
#show link: set text(fill: rgb("#0366d6"))

// Task-list helper used by the converter (§6.2).
#let task(checked) = if checked { "[x]" } else { "[ ]" }

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

**User template override:** `template_path` in `RenderOptions`. When set, read the file; if it doesn't exist or fails a probe compile (§7.4), return a descriptive error and keep the previous template. Never fall back silently.

**Fonts:** the engine must be given at least one font. Embed one via `include_bytes!` — e.g. DejaVu Sans (permissive license) — and pass it to the engine (§6.4). CJK and wider font coverage are future work (§13); if the engine builder offers a system-font option, prefer it when the user provides `template_path` (custom templates may use any system font).

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
    // Doc: Output; annotate the call so inference picks PagedDocument.
    let doc = engine.compile().output.map_err(|e| EngineError::Compile(render_error(&e)))?;
    Ok(doc)
}
```

**Render preview pages (SVG):**

```rust
pub fn compile_preview(source: &str) -> Result<PreviewOutcome, EngineError> {
    let doc = compile_document(source)?;
    // typst-svg 0.15: svg(page: &Page, opts: &SvgOptions) -> String — takes
    // the whole page, not `&page.frame` as in older versions.
    let pages = doc
        .pages
        .iter()
        .map(|page| typst_svg::svg(page, &Default::default()))
        .collect::<Vec<String>>();
    Ok(PreviewOutcome { pages, warnings: collect_warnings(&doc) })
}
```

**Export PDF:**

```rust
pub fn compile_pdf(source: &str) -> Result<Vec<u8>, EngineError> {
    let doc = compile_document(source)?;
    typst_pdf::pdf(&doc, &Default::default())
        .map_err(|e| EngineError::Pdf(e.to_string()))
}
```

**API-verification checklist** (mandatory before coding — these signatures drift between Typst releases). Verified facts for the 0.15/0.16 line (from docs.rs, June–July 2026), re-confirm against the exact locked version:

1. `TypstEngine::builder()` → `TypstTemplateEngineBuilder`; `.main_file(source: impl IntoSource)` takes source *content*; `.fonts([&[u8]])` takes byte slices; `.build()` → `TypstEngine<TypstTemplateMainFile>`. ✓ (docs.rs typst-as-lib 0.16.0)
2. `engine.compile()` (no args when a main file is set) returns `Warned<Result<Doc, TypstAsLibError>>` where `Doc: Output`; unwrap via `.output`. `Warned.output` holds `Result<Doc, TypstAsLibError>`; the error type is `TypstAsLibError`, **not** a `Vec<SourceDiagnostic>` as in older versions — map it with the world to render diagnostics (below). ✓
3. `typst_pdf::pdf(&doc, &PdfOptions) -> Result<Vec<u8>, _>` — returns bytes; options struct is `PdfOptions` (has `Default`). ✓
4. `typst_svg::svg(page: &Page, opts: &SvgOptions) -> String` — first argument is a page `Page` (from `doc.pages`), not a frame. ✓ (docs.rs typst-svg 0.15.1)
5. `PagedDocument` lives in typst-layout 0.15 (`typst_layout::document::PagedDocument`, re-exported as `typst_layout::PagedDocument`); `doc.pages: Vec<Page>`. Confirm the exact re-export path against the locked typst-layout version.

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
  "permissions": ["core:default", "dialog:default"]
}
```

`dialog:default` covers open/save/message dialogs. Do **not** add `core:webview` or any filesystem plugin permissions — file I/O happens inside Rust commands via `std::fs` (§7.4), not through frontend plugins. This is the principle of least privilege.

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

/// Live preview: markdown + options → per-page SVG strings.
#[tauri::command]
pub async fn render_preview(
    app: AppHandle,
    payload: RenderPayload,        // { markdown, options, seq }
) -> Result<PreviewOutcome, String> {
    let source = build_source(&app, &payload)?;         // pure logic, see below
    let outcome = tauri::async_runtime::spawn_blocking(move || engine::compile_preview(&source))
        .await
        .map_err(|e| format!("Preview task failed: {e}"))??;
    Ok(PreviewOutcome { pages: outcome.pages, warnings: outcome.warnings, seq: payload.seq })
}

/// Export: markdown + options → system save dialog → write PDF to disk.
#[tauri::command]
pub async fn export_pdf(app: AppHandle, payload: RenderPayload) -> Result<serde_json::Value, String> {
    let source = build_source(&app, &payload)?;
    let pdf = tauri::async_runtime::spawn_blocking(move || engine::compile_pdf(&source))
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

/// Validate a custom template by compiling a probe document (§6.3).
#[tauri::command]
pub async fn validate_template(path: String) -> Result<serde_json::Value, String> {
    let source = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read template: {e}"))?;
    let probe = template::compose(&source, &TemplateArgs {
        paper: "a4", margin_x_mm: 20.0, margin_y_mm: 25.0,
        text_align: "left", title: None, author: None, body: "probe",
    });
    engine::compile_preview(&probe)?;                    // must not error
    Ok(serde_json::json!({ "valid": true }))
}
```

**`build_source` — shared pure logic** (kept in `template.rs` so it is unit-testable without Tauri):

```rust
pub fn build_source(
    template_source: &str,          // already read from default or user path
    options: &RenderOptions,
    markdown: &str,
) -> Result<String, String> {
    let converted = md_to_typst(markdown, options.flavor).map_err(|e| e.to_string())?;
    Ok(compose(template_source, &TemplateArgs {
        paper: options.paper.typst_name(),
        margin_x_mm: options.margins.left_mm.min(options.margins.right_mm),
        margin_y_mm: options.margins.top_mm.min(options.margins.bottom_mm),
        text_align: match options.text_align { TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Justified => "justified" },
        title: converted.title.as_deref(),
        author: converted.author.as_deref(),
        body: &converted.body,
    }))
}
```

(Margins use the *smaller* of the two horizontal/vertical values for the `x`/`y` shorthand — or, cleaner, pass all four to the template and let `article` accept `top/bottom/left/right`. Pick one and keep the template and `TemplateArgs` in sync; the test suite must pin the chosen behaviour.)

### 7.5 Async & concurrency notes

- Typst compilation is CPU-bound and can take tens of milliseconds on large documents. Run it in `tauri::async_runtime::spawn_blocking` (Tokio's `spawn_blocking`) so the IPC handler doesn't stall the runtime. Readers comfortable with Tokio will recognize this as the standard pattern for CPU work in an async runtime.
- **Stale responses:** rapid typing can interleave responses. The frontend includes a monotonically increasing `seq` in each `render_preview` payload; the command echoes it back; the frontend drops any response whose `seq` is older than the last request it sent (§8.3). This is the cheapest correct solution; a real cancellation mechanism is future work.
- No shared `State` in v1 (each compile builds a fresh engine). If profiling shows engine construction cost matters, cache a `Mutex<TypstEngine<...>>` in `tauri::State` and rebuild it only when the template changes (§13).

### 7.6 `lib.rs` registration

```rust
mod commands; mod markdown; mod options; mod typst_engine;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::render_preview,
            commands::export_pdf,
            commands::open_markdown,
            commands::save_markdown,
            commands::validate_template,
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
import "@fontsource/pt-sans-narrow/400.css";
import "@fontsource/pt-sans-narrow/700.css";
import { MantineProvider, createTheme } from "@mantine/core";
import App from "./App.jsx";

const theme = createTheme({
  fontFamily: "PT Sans Narrow, sans-serif",
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
- **Compile errors** (e.g. broken custom template, unsupported construct): show the first ~5 diagnostics in the banner; keep the previous preview.
- **Warnings** (missing font, etc.): shown in the status bar as `N warning(s)` and in a collapsible list; never fail the render.
- **Missing/custom template:** errors on read or probe compile are surfaced with the offending path.
- **Export cancel:** the dialog returning `None` is *not* an error (the JSON includes `canceled: true`); the UI must not show a banner for it.

## 10. Testing (TDD)

Follow red-green-refactor. Because `markdown/` and `typst_engine/` are plain Rust (no `tauri` imports), tests run with plain `cargo test` — no app startup needed. Suggested suite:

**`markdown/convert.rs` tests (table-driven — the bulk of the value):**

- `escapes hash/brackets/asterisks/underscores/dollar/backslash in text`
- `maps heading levels to =, ==, …`
- `maps bold/italic/strikethrough`
- `maps inline code and code blocks (with language, without language, containing backticks)`
- `maps links with and without label, URL containing quotes`
- `maps ordered and unordered lists, nested lists`
- `maps task list items to #task(...)`
- `maps GFM table to #table(...)` (only when flavor = GFM/Custom; in CommonMark comrak yields paragraphs)
- `maps footnotes and math in Custom flavor; leaves them as text otherwise`
- `strips HTML tags and keeps text` (lossy path)
- `parses YAML front matter title/author; ignores other keys` (no title/author → `ConvertedDocument` with `None`; title containing quotes → valid escaped Typst string)
- `empty input returns []`

**`typst_engine/template.rs` tests:**

- `compose emits template + article invocation with expected args`
- `compose passes body through verbatim (already escaped)`
- `compose emits title: none / author: none when absent; quoted string literals when present`
- `build_source maps paper/margins/alignment to template args; forwards title/author`
- `template contract: default template defines article with required params` (compile probe)

**Frontend (optional in M1, required later):** the pure modules (`lib/presets.js`, `lib/settings.js`) are testable with Vitest. Component tests (React Testing Library) are not required for v1; the seq guard and debounce are simple enough to review by hand — keep them in `App.jsx` so they are auditable.

**`typst_engine/engine.rs` tests (slow but few — mark with `#[ignore]` or keep tiny):**

- `compile_preview returns at least one SVG containing "<svg"`
- `compile_pdf bytes start with "%PDF" and are non-empty`
- `compile error path yields diagnostics, not a panic`
- `custom template that omits article fails probe compile`

**Command-layer tests (optional, Tauri 2):** `tauri::test::mock_builder` / `mock_app` can invoke commands in-process; at minimum assert that `build_source` (shared logic) is covered by the unit tests above. Full IPC testing is not required for v1.

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
| **M1 — Converter** | `markdown/convert.rs` + `escape.rs` (TDD) | Full unit suite green; CommonMark/GFM/Custom flavour differences verified by tests. |
| **M2 — Engine** | `typst_engine/` — template, compose, compile | `compile_preview(sample)` returns ≥1 SVG containing `<svg`; `compile_pdf` writes a file starting with `%PDF`; error paths tested. |
| **M3 — UI loop** | Editor + preview + debounce + seq guard | Typing updates preview ≤300 ms after pause; flavor switch changes output (e.g. table renders only in GFM/Custom); stale responses are dropped. |
| **M4 — Files & export** | Open/Save/Export commands wired to toolbar | Open loads a file; Save persists; Export writes a PDF that opens in a viewer; cancel shows no error. |
| **M5 — Settings & polish** | localStorage settings, template override + validation, warnings UI, error banner | Settings survive restart; custom template accepted only after probe validation; warnings displayed; empty doc renders one blank page. |

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
- Only one embedded font by default; no CJK coverage out of the box.
- Remote images unsupported; images must be local files reachable through the world's file resolver.
- No LaTeX math macros (`tex2typst-rs` could be added, as `mdxport` does).
- No incremental compilation — full recompile per change; large (50+ page) documents may show latency.
- SVG preview text is not selectable (inline SVG). A canvas/DOM renderer via `typst.ts` would restore selection but introduces WASM + version-pinning concerns (see §2).

Future work, in order of leverage:

1. **Incremental preview** — adopt `tinymist`/`typst-preview`-style patched SVG deltas (comemo memoisation) or stream pages via Tauri events instead of one big return value.
2. **Reusable engine** — cache `TypstEngine` in `State`, rebuild on template change only; use `with_world`/`sys.inputs` to avoid re-reading fonts.
3. **Syntax highlighting** in code blocks (syntect pre-wrapping or Typst packages).
4. **HTML export** via `typst-html` (the `mdcast` project demonstrates this).
5. **Table of contents, page numbers, running headers** — template-driven, zero core changes.
6. **`Export .typ`** button (show the composed source) — cheap, great for debugging and power users.
7. **SyncTeX-like preview↔source jumping** (click page → cursor).

## 14. References

- Tauri 2: configuration (`https://v2.tauri.app/reference/config/`), capabilities (`https://v2.tauri.app/security/capabilities/`), command docs (`https://v2.tauri.app/develop/calling-rust/`), dialog plugin (`https://v2.tauri.app/plugin/dialog/`), testing (`https://docs.rs/tauri/latest/tauri/test/`).
- Typst 0.15: `typst` (`https://docs.rs/typst`), `typst-pdf`, `typst-svg`, `typst-as-lib` (`https://docs.rs/typst-as-lib` — README has the `TypstEngine` example used in §6.4).
- Typst markup reference: `https://typst.app/docs/reference/` (text, raw, link, image, table, blockquote, strike, footnote, set rules).
- comrak: `https://docs.rs/comrak` (AST node reference for §6.2).
- Reference projects worth reading: `mdxport` (comrak → typst → PDF, in-process), `mdcast` (typst-as-lib integration, typst-html), `typst-preview` (incremental SVG preview architecture).
