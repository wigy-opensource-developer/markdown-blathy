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
│  CodeMirror 6 (markdown)   → debounce 200ms → invoke("render_preview")    │
│  Preview pane: <div> of inline SVG pages (one per printed page)          │
│  Toolbar / status bar / error banner                                     │
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

comrak = "0.35"            # CommonMark/GFM parser
typst = "0.15"             # the Typst compiler (library)
typst-pdf = "0.15"         # PDF export backend
typst-svg = "0.15"         # SVG export backend (preview pages)
typst-as-lib = "0.15"      # high-level TypstWorld wrapper (avoids hand-writing World)
```

**Version pinning rule:** `typst`, `typst-pdf`, `typst-svg`, `typst-as-lib` must **all share the same minor version** (they are released in lockstep). Pin exact versions in `Cargo.lock`. Before implementing, read the docs.rs pages for these four crates at the pinned version — the API names below are correct for the 0.15 line but you must confirm signatures against the exact version you lock (see §6.4).

### 4.2 Frontend (`package.json`)

```json
{
  "name": "tauri-md-editor",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "vite build",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@codemirror/state": "^6",
    "@codemirror/view": "^6",
    "@codemirror/language": "^6",
    "@codemirror/commands": "^6",
    "@codemirror/lang-markdown": "^6"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "vite": "^6"
  }
}
```

No CSS framework. No frontend Markdown renderer — the preview is Typst SVG, not HTML.

### 4.3 System prerequisites (per platform)

- **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, plus a C linker and `pkg-config` (see Tauri docs). `build-essential`, `libssl-dev` may also be required.
- **macOS**: Xcode Command Line Tools.
- **Windows**: WebView2 runtime (preinstalled on Windows 11 / updated Windows 10).

## 5. Repository structure

```
tauri-md-editor/
├── package.json
├── vite.config.js               # port 1420, strictPort (standard Tauri template)
├── index.html                   # single page: toolbar + split panes + status bar
├── src/                         # ── frontend ──
│   ├── main.js                  # bootstrap: CM editor, preview, IPC wiring
│   ├── editor.js                # CodeMirror 6 setup + debounced change hook
│   ├── preview.js               # SVG page rendering, stale-response guard
│   ├── settings.js              # localStorage-backed settings
│   ├── api.js                   # thin invoke() wrappers + payload types
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
│           ├── template.rs      # default.typ embedding, compose()
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

```rust
pub fn comrak_options(flavor: Flavor) -> comrak::Options {
    // comrak ≥0.30 uses ExtensionOptionsBuilder; verify exact field names
    // against the docs for the pinned version.
    let ext = comrak::ExtensionOptionsBuilder::default()
        .strikethrough(matches!(flavor, Flavor::Gfm | Flavor::Custom))
        .table(matches!(flavor, Flavor::Gfm | Flavor::Custom))
        .autolink(matches!(flavor, Flavor::Gfm | Flavor::Custom))
        .tasklist(matches!(flavor, Flavor::Gfm | Flavor::Custom))
        .footnotes(flavor == Flavor::Custom)
        .math_dollars(flavor == Flavor::Custom)
        .build()
        .expect("valid extension options");
    comrak::Options { extension: ext, ..comrak::Options::default() }
}
```

### 6.2 Markdown → Typst converter (`markdown/convert.rs`) — the core of the app

Signature:

```rust
pub fn md_to_typst(markdown: &str, flavor: Flavor) -> Result<String, ConvertError>
```

Implementation sketch: parse with `comrak::parse_document(&arena, markdown, &opts)` and recursively walk the `NodeValue` tree. **Never call `comrak::markdown_to_html`.** Map nodes as follows:

| Markdown node (comrak `NodeValue`) | Typst output |
|---|---|
| `Document` | concatenation of children (blocks separated by blank lines) |
| `Heading { level, .. }` | `"="` × level + space + inline content |
| `Paragraph` | inline content, followed by `\n` |
| `Text` | **escaped** text (see table below) |
| `SoftBreak` | space |
| `LineBreak` | `\ ` (escaped space = hard break in Typst) |
| `Strong` | `*…*` |
| `Emph` | `_…_` |
| `Strikethrough` | `#strike[…]` |
| `Code` (inline) | backtick raw: `` `code` `` (see backtick rule below) |
| `CodeBlock { info, .. }` | fenced raw block: ``` `````lang````` ⏎ code ⏎ ````` ``` |
| `Link { url, .. }` | `#link("url")[label]` (URL-escape `"` inside the string) |
| `Image { url, .. }` | `#figure(image("url", width: 80%), caption: [ …optional alt text… ])` |
| `List { ordered, start, .. }` | `- ` / `1. ` per item |
| `Item` | recurse children |
| `TaskItem { checked }` | `#task(true/false)` + item text (template defines `#task`, §6.3) |
| `BlockQuote` | `#blockquote[…children…]` (native since Typst 0.13; else fallback `#quote[block(…)]`) |
| `Table` / `TableHeader` / `TableRow` / `TableCell` | `#table(columns: (auto, …), [cell], [cell], …)`; header cells wrapped in `*…*` |
| `ThematicBreak` | `#v(0.6em) #line(length: 100%, stroke: 0.5pt) #v(0.6em)` |
| `FootnoteReference` / `FootnoteDefinition` | `#footnote[…]` |
| `Math` | passthrough: inline `$…$`, block `$ … $` (LaTeX macros are **not** converted in v1; document the limitation) |
| `FrontMatter` | parse YAML `title` / `author` only → template args (§6.3); everything else ignored |
| `HtmlInline` / `HtmlBlock` | **lossy**: emit the plain-text content of child text nodes, drop tags. Never pass HTML through to Typst |

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
- **Empty document:** return `[]` (an empty content block) so compile always succeeds.

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

// Main entry point. The Rust side always appends: #article(...)[body]
#let article(
  paper: "a4",
  margin-x: 20mm,
  margin-y: 25mm,
  text-align: "left",
  body,
) = {
  set page(paper: paper, margin: (x: margin-x, y: margin-y))
  if text-align == "center" { set align(center) }
  else if text-align == "justified" { set par(justify: true) }
  body
}
```

**Composition** — this is the only place where Rust builds a Typst source string:

```rust
pub struct TemplateArgs<'a> {
    pub paper: &'a str,          // already the Typst name, e.g. "a4"
    pub margin_x_mm: f32,
    pub margin_y_mm: f32,
    pub text_align: &'a str,     // "left" | "center"
    pub body: &'a str,           // output of md_to_typst (escaped markup)
}

pub fn compose(template_source: &str, args: &TemplateArgs) -> String {
    format!(
        "{template_source}\n\n#article(\n  paper: {paper:?},\n  margin-x: {x}mm,\n  margin-y: {y}mm,\n  text-align: {align:?},\n)[\n{body}\n]",
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
- Margins are formatted with `format!("{}", value)` — values come from a fixed enum/preset in v1, never free text.
- **The body is never interpolated into template source unescaped** — the converter already escaped it (§6.2), and it is placed *inside* a content block, which is the only place markup is valid. This mirrors the "escape on input, trust on output" rule.

**User template override:** `template_path` in `RenderOptions`. When set, read the file; if it doesn't exist or fails a probe compile (§7.4), return a descriptive error and keep the previous template. Never fall back silently.

**Fonts:** the engine must be given at least one font. Embed one via `include_bytes!` — e.g. DejaVu Sans (permissive license) — and pass it to the engine (§6.4). CJK and wider font coverage are future work (§13); if the engine builder offers a system-font option, prefer it when the user provides `template_path` (custom templates may use any system font).

### 6.4 Compile & export (`typst_engine/engine.rs`)

Uses `typst-as-lib` (v0.15.5) so implementers do **not** hand-write a `typst::World`. Two entry points, both built on one helper:

```rust
use typst_as_lib::TypstEngine;

/// Compile a complete Typst source and return the typed document.
fn compile_document(source: &str) -> Result<typst::doc::Document, EngineError> {
    let engine = TypstEngine::builder()
        .main_file(source)                          // source CONTENT, not a path
        .fonts([&*FONT_BYTES])                      // include_bytes! of one embedded font
        .build();

    // `compile()` returns Warned<Result<Document, Vec<SourceDiagnostic>>>.
    let warned = engine.compile();
    let doc = warned.output.map_err(|diags| EngineError::Compile(render_diagnostics(&diags)))?;
    Ok(doc)
}
```

**Render preview pages (SVG):**

```rust
pub fn compile_preview(source: &str) -> Result<PreviewOutcome, EngineError> {
    let doc = compile_document(source)?;
    let pages = doc
        .pages
        .iter()
        .map(|page| typst_svg::svg(&page.frame, &Default::default()))
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

**API-verification checklist** (mandatory before coding — these signatures drift between Typst releases):

1. `TypstEngine::builder().main_file(...).fonts(...).build()` — confirm `main_file` takes source *content* and `fonts` takes byte slices.
2. `engine.compile()` / `compile_with_input(...)` — confirm the `Warned`/`.output` shape.
3. `typst_pdf::pdf(&doc, &PdfOptions)` — confirm it returns `Vec<u8>` (or io Result) and the options type name.
4. `typst_svg::svg(&frame, ...)` — confirm the argument type (a page `Frame`; some versions take the document and page index) and that it returns a `String`. Adjust the loop accordingly.

**Error rendering:** map the first ~5 `SourceDiagnostic`s to `"line {span.linenum()}: {message}"`-style strings (use the diagnostic's `span` with the `Source` from the world; typst-as-lib exposes the world via `with_world` if needed — see §13 for the low-level path). Warnings are a separate list returned to the UI.

## 7. Tauri shell

### 7.1 Bootstrap

1. Scaffold: `npm create tauri-app@latest` → name `tauri-md-editor`, frontend template **Vanilla (Vite)**, keep the default `src/` + `src-tauri/` layout. Or scaffold manually with the files in this spec.
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
        paper: "a4", margin_x_mm: 20.0, margin_y_mm: 25.0, text_align: "left", body: "probe",
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
    let body = md_to_typst(markdown, options.flavor).map_err(|e| e.to_string())?;
    Ok(compose(template_source, &TemplateArgs {
        paper: options.paper.typst_name(),
        margin_x_mm: options.margins.left_mm.min(options.margins.right_mm),
        margin_y_mm: options.margins.top_mm.min(options.margins.bottom_mm),
        text_align: match options.text_align { TextAlign::Left => "left", TextAlign::Center => "center", TextAlign::Justified => "justified" },
        body: &body,
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

### 8.1 Page structure (`index.html`)

The HTML/CSS target is a beginner-intermediate level, so this uses plain flexbox — one toolbar row on top, two columns below, a status row at the bottom:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Markdown to PDF</title>
  </head>
  <body>
    <header id="toolbar">
      <button data-action="new">New</button>
      <button data-action="open">Open</button>
      <button data-action="save">Save</button>
      <span class="sep"></span>
      <label>Flavor
        <select id="flavor">
          <option value="commonmark">CommonMark</option>
          <option value="gfm" selected>GFM</option>
          <option value="custom">Custom</option>
        </select>
      </label>
      <label>Paper
        <select id="paper">
          <option value="a4" selected>A4</option>
          <option value="a5">A5</option>
          <option value="letter">Letter</option>
          <option value="legal">Legal</option>
        </select>
      </label>
      <label>Margins
        <select id="margins">
          <option value="normal" selected>Normal (20/25mm)</option>
          <option value="narrow">Narrow (10/15mm)</option>
          <option value="custom">Custom…</option>
        </select>
      </label>
      <label>Align
        <select id="align">
          <option value="left" selected>Left</option>
          <option value="center">Center</option>
        </select>
      </label>
      <span class="spacer"></span>
      <button data-action="template" title="Choose a custom .typ template">Template…</button>
      <button data-action="export" id="export" class="primary">Export PDF</button>
    </header>

    <main id="panes">
      <section id="editor-pane"><div id="editor"></div></section>
      <section id="preview-pane">
        <div id="preview-scroll"><div id="preview"></div></div>
      </section>
    </main>

    <footer id="status"><span id="status-text">Ready</span></footer>
    <div id="error-banner" hidden></div>

    <script type="module" src="/src/main.js"></script>
  </body>
</html>
```

### 8.2 Editor (`editor.js`)

CodeMirror 6 is ESM-only and installed via npm (bundled by Vite). Minimal setup with a markdown language and a change hook:

```js
import { EditorView, keymap, placeholder } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { syntaxHighlighting, defaultHighlightStyle } from "@codemirror/language";

export function createEditor(parent, { onChange, onSave }) {
  let view;
  const state = EditorState.create({
    doc: "# Hello\n\nStart typing…",
    extensions: [
      history(),
      markdown(),
      syntaxHighlighting(defaultHighlightStyle),
      placeholder("Write markdown here…"),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) onChange(u.state.doc.toString());
      }),
      EditorView.theme({
        "&": { height: "100%", fontSize: "14px" },
        ".cm-scroller": { overflow: "auto" },
      }),
    ],
  });
  view = new EditorView({ state, parent });
  return {
    view,
    getDoc: () => view.state.doc.toString(),
    setDoc: (text) => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } }),
    focus: () => view.focus(),
  };
}
```

### 8.3 Preview (`preview.js`)

Renders the SVG pages returned by `render_preview` as inline DOM (safe: our compiler produced them) and implements the stale-response guard:

```js
let lastSeq = 0;

export function renderPreview(container, result) {
  if (result.seq < lastSeq) return;      // stale response — drop it
  lastSeq = result.seq;
  container.innerHTML = result.pages.map((svg, i) =>
    `<div class="page" data-page="${i + 1}">${svg}</div>`
  ).join("");
}

export function renderError(message) { /* sets #error-banner text */ }
```

The pages update wholesale on every compile. Incremental / patch-based updates (typst-preview style) are future work (§13).

### 8.4 `main.js` wiring and `api.js`

`api.js` — one wrapper per command (using `@tauri-apps/api` v2):

```js
import { invoke } from "@tauri-apps/api/core";

export const renderPreview = (payload) => invoke("render_preview", { payload });
export const exportPdf = (payload) => invoke("export_pdf", { payload });
export const openMarkdown = () => invoke("open_markdown");
export const saveMarkdown = (path, content) => invoke("save_markdown", { path, content });
export const validateTemplate = (path) => invoke("validate_template", { path });
```

`main.js` — the debounce loop and the settings bridge:

```js
import { createEditor } from "./editor.js";
import { renderPreview, renderError } from "./preview.js";
import * as api from "./api.js";
import { loadSettings, saveSettings } from "./settings.js";

const settings = loadSettings();            // localStorage-backed (§8.6)
const $ = (sel) => document.querySelector(sel);
let seq = 0;
let debounceTimer;

const state = {
  markdown: "",
  filePath: null,
  templatePath: settings.templatePath ?? null,
};

async function refreshPreview() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(async () => {
    const seqId = ++seq;
    try {
      const result = await api.renderPreview({
        markdown: state.markdown,
        options: currentOptions(),
        seq: seqId,
      });
      renderPreview($("#preview"), result);
      $("#status-text").textContent = `${result.pages.length} page(s)`;
    } catch (err) {
      renderError(String(err));
      $("#status-text").textContent = "Preview failed";
    }
  }, 200);
}

function currentOptions() {
  return {
    flavor: $("#flavor").value,
    paper: $("#paper").value,
    margins: settings.margins,          // { topMm, bottomMm, leftMm, rightMm }
    textAlign: $("#align").value,
    templatePath: state.templatePath,
  };
}

// ── toolbar wiring ─────────────────────────────────────────────────────────
$("#flavor").onchange = (e) => { settings.flavor = e.target.value; saveSettings(settings); refreshPreview(); };
$("#paper").onchange   = (e) => { settings.paper = e.target.value; saveSettings(settings); refreshPreview(); };
$("#align").onchange   = (e) => { settings.textAlign = e.target.value; saveSettings(settings); refreshPreview(); };
$("#margins").onchange = (e) => { settings.marginPreset = e.target.value; /* update settings.margins from preset table */ saveSettings(settings); refreshPreview(); };

document.querySelector('[data-action="export"]').onclick = async () => {
  try { await api.exportPdf({ markdown: state.markdown, options: currentOptions() }); }
  catch (err) { renderError(String(err)); }
};
document.querySelector('[data-action="open"]').onclick = async () => {
  const res = await api.openMarkdown();
  if (res.canceled) return;
  if (res.error) return renderError(res.error);
  state.markdown = res.content; state.filePath = res.path;
  editor.setDoc(res.content);
};
document.querySelector('[data-action="save"]').onclick = async () => {
  const res = await api.saveMarkdown(state.filePath, state.markdown);
  if (!res.canceled && !res.error) state.filePath = res.path;
};
document.querySelector('[data-action="template"]').onclick = async () => {
  // open a file dialog with a filter for .typ (reuse a Rust command, or
  // tauri-plugin-dialog from the frontend) then validate via api.validateTemplate
  // and set state.templatePath on success.
};

// ── boot ────────────────────────────────────────────────────────────────────
const editor = createEditor($("#editor"), {
  onChange: (md) => { state.markdown = md; refreshPreview(); },
  onSave: () => document.querySelector('[data-action="save"]').click(),
});
state.markdown = editor.getDoc();
refreshPreview();
```

### 8.5 Styles (`styles.css`)

Flexbox fundamentals: `display:flex` on the panes row, `flex:1` on each pane, `flex-direction:column` on the editor pane so the CM element fills height.

```css
* { box-sizing: border-box; }
html, body { height: 100%; margin: 0; }
body { display: flex; flex-direction: column; font-family: system-ui, sans-serif; }

#toolbar { display: flex; align-items: center; gap: 8px; padding: 6px 10px;
           border-bottom: 1px solid #ccc; background: #f6f8fa; }
#toolbar .spacer { flex: 1; }
#toolbar .sep { width: 1px; align-self: stretch; background: #ccc; margin: 0 4px; }
button.primary { background: #0366d6; color: #fff; border: none; border-radius: 4px; padding: 4px 12px; }

#panes { flex: 1; display: flex; min-height: 0; }
#editor-pane  { flex: 1; min-width: 0; display: flex; flex-direction: column; }
#editor       { flex: 1; min-height: 0; overflow: hidden; }
#preview-pane { flex: 1; min-width: 0; background: #525659; overflow: auto; }

#preview { padding: 24px; }
/* Each SVG page is a white sheet with a shadow, centered, sized to the pane width. */
.page { background: #fff; box-shadow: 0 2px 8px rgba(0,0,0,.35);
        width: 100%; max-width: 820px; margin: 0 auto 24px; padding: 0; }
.page svg { display: block; width: 100%; height: auto; }

#status { display: flex; gap: 12px; padding: 4px 10px; border-top: 1px solid #ccc;
          font-size: 12px; color: #555; background: #f6f8fa; }
#error-banner { position: fixed; top: 8px; left: 50%; transform: translateX(-50%);
                background: #d93025; color: #fff; padding: 8px 16px; border-radius: 4px; }
```

Key detail for the preview: Typst emits SVGs with an intrinsic pixel size (e.g. A4 @ 96dpi ≈ 794×1123). `width:100%; height:auto` scales them to the pane while preserving aspect ratio, so pages behave like a real document viewer.

### 8.6 Settings (`settings.js`)

v1 persists to `localStorage` under one JSON key (e.g. `tauri-md-editor.settings`): `{ flavor, paper, marginPreset, margins, textAlign, templatePath, editorFontSize }`. Provide a preset table for margins (Normal / Narrow / Custom). `tauri-plugin-store` (a file-backed store) is an acceptable alternative but not required in v1.

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
- `parses YAML front matter title/author; ignores other keys`
- `empty input returns []`

**`typst_engine/template.rs` tests:**

- `compose emits template + article invocation with expected args`
- `compose passes body through verbatim (already escaped)`
- `build_source maps paper/margins/alignment to template args`
- `template contract: default template defines article with required params` (compile probe)

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
| **M0 — Scaffold** | Project skeleton (Vite + Tauri + plugins) | `npm run tauri dev` opens an empty window; `cargo test` runs (0 tests or stubs). |
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
