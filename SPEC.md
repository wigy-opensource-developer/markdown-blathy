# Tauri MD Editor — Product Specification

| | |
|---|---|
| **Status** | v1.0 |
| **Target** | Desktop app (Windows / macOS / Linux) built with Tauri 2 + Rust + Typst |
| **Reader** | Implementer (AI agent or human). Assumes solid Rust & Tokio; beginner–intermediate Tauri 2 and HTML/CSS. |

> **Note:** This is an implementation specification — it describes how the app is built, not how to use it. For a user-facing overview see [README.md](README.md).

## Table of contents

1. [Product overview](#1-product-overview)
2. [Non-goals (v1)](#2-non-goals-v1)
3. [Architecture](#3-architecture)
4. [Flavor support](#4-flavor-support)
5. [Milestones](#5-milestones)
6. [Limitations](#6-limitations)
7. [References](#7-references)

---

## 1. Product overview

A desktop **Markdown editor with live preview and on-demand PDF export**. The preview and the exported PDF are produced by **the same Typst compilation pipeline**, which guarantees the preview is pixel-identical to the PDF (true WYSIWYG).

User-visible behaviour:

1. Split-screen window: **Markdown editor** (left) and **live preview** (right).
2. The preview updates automatically while typing (debounced), rendering pages with print margins, exactly as they will appear in the PDF.
3. A toolbar offers: New / Open / Save Markdown files, a **Document** settings popover (title, author, auto-heading level, and the Markdown extensions/flavor), paper size, margins, text alignment, and **Export PDF** (system save dialog). Flavor is a per-document setting (front matter), not a toolbar select; the default is Custom (every extension on).
4. The default document style is provided by an embedded Typst template; users may supply their own `.typ` template.
5. Compile warnings (e.g. unknown fonts) are surfaced in the UI, not swallowed.

## 2. Non-goals (v1)

- No incremental compilation (tinymist-style). Full recompile per change is fine for typical documents (< ~50 pages).
- No client-side Typst WASM (`typst.ts`). All compilation happens in Rust; this avoids version drift between preview and PDF.
- No syntax highlighting inside code blocks in the PDF (Typst has no built-in highlighter).
- No raw-HTML support in Markdown (lossy by design).
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

**Design rule:** `markdown/` and `typst_engine/` must not import `tauri`. The `#[tauri::command]` functions are thin, serialization-only wrappers. This makes the core unit-testable without a running app.

**Commands are stateless in v1** — each invoke recompiles the document from the current inputs. Perf is acceptable: Typst compiles a typical page in single-digit milliseconds, and the engine cache + memoization keep per-render cost low.

## 4. Flavor support

A *flavor* is a named preset of extension flags. The effective behavior is a
set of feature flags; a document can override its flavor and flip individual
extensions via front matter (see below).

| Feature | CommonMark | GitHub | Custom |
|---|---|---|---|
| strikethrough | — | ✅ | ✅ |
| tables | — | ✅ | ✅ |
| heading IDs (`{#id}`) | ✅ | ✅ | ✅ |
| task lists | — | ✅ | ✅ |
| footnotes | — | — | ✅ |
| math (`$...$`) | — | — | ✅ |
| highlight (`==text==`) | — | — | ✅ |
| subscript (`~text~`) | — | — | ✅ |
| superscript (`^text^`) | — | — | ✅ |
| description lists | — | — | ✅ |
| page breaks (`<!-- pagebreak -->`) | — | — | ✅ |

**Front matter is parsed in every flavor.** A leading `---` block of
`key: value` lines carries the document settings — `title`, `author`,
`autoHeadingLevel`, `flavor`, and `ext_*` extension overrides — which
override the toolbar options. Only `key: value` syntax is supported (no YAML
crate); unknown keys warn with a "did you mean …?" suggestion. When the
block is not valid front matter (e.g. a thematic break mistaken for one), the
leading `---` is ignored with a warning and the content still renders.

**Extension overrides.** On top of the `flavor:` preset, each extension can be
toggled with an `ext_<name>: true|false` key (e.g. `ext_footnotes: true`).
When the app writes front matter it uses the XOR-minimal encoding: the preset
with the fewest flips, plus only the differing flags.

**Explicit bare links** (`<https://example.com>`) are supported in **all flavors** — this is standard CommonMark, not an extension. The `autolink` extension (automatic bare-URL linking) is **off in every flavor**.

## 5. Milestones

| Milestone | Deliverable | Status |
|---|---|---|
| **M0 — Scaffold** | Project skeleton (Vite + React 19 + Tauri + plugins) | ✅ Done |
| **M1 — Converter** | `markdown/convert.rs` + `escape.rs` (TDD) | ✅ Done |
| **M2 — Engine** | `typst_engine/` — template, compose, compile | ✅ Done |
| **M3 — UI loop** | Editor + preview + debounce + seq guard | ✅ Done |
| **M4 — Files & export** | Open/Save/Export commands wired to toolbar | ✅ Done |
| **M5 — Settings & polish** | localStorage settings, template override + validation | ✅ Done |
| **M6 — Engine cache & fonts** | Reusable engine + system/template-side fonts | ✅ Done |
| **M7 — Logging & extended syntax** | `tracing` + SVG internal links + heading IDs + new constructs | 🚧 In progress — tracing, links and heading IDs done; new constructs remain |

See **IMPLEMENTATION.md** for acceptance criteria and test plans per milestone.

## 6. Limitations

- Raw HTML in Markdown is stripped to text, except the `<!-- pagebreak -->` marker (Custom flavor), which becomes a page break.
- No syntax highlighting inside code blocks in the PDF.
- Only the embedded DejaVu family by default (Sans regular + bold, Sans Mono for code, Math TeX Gyre for math); no CJK coverage out of the box. Custom templates can reference system fonts and ship font files next to the `.typ` file.
- **System-font naming is typst's, not fontconfig's** — typst strips styling suffixes from family names, so a font whose *name* carries the styling may be unreachable. Use the trimmed family name or a font without a styling suffix.
- Remote images unsupported; images must be local files reachable through the world's file resolver.
- No LaTeX math macros.
- No incremental compilation — full recompile per change.
- **Linux UI text is grayscale-antialiased** — WebKitGTK renders text onto composited transparent layers. The Typst SVG preview is vector paths and unaffected.
- **Non-UTF-8 filenames on legacy-locale Linux** — paths travel over IPC as JSON strings (UTF-8), so a filename in e.g. latin-2 bytes is echoed back lossily. File *contents* still load and writes still go to the correct file.
- SVG preview text is not selectable (inline SVG).
- **Emoji shortcodes** (`:smile:`) are not expanded — the author pastes actual emoji characters.
- **Automatic URL linking** is not enabled — users write explicit bare links (`<https://example.com>`) when they want a link.
- **Table of contents**: manual TOCs work via heading IDs (a list of `[Title](#slug)` links). An auto-marker `[[_TOC_]]` (GitLab convention) triggers auto-TOC in Custom flavor.
- **Internal links in the SVG preview are inert on typst 0.15** — the paged introspector has no anchors, so footnote/heading/TOC links carry no `href` in the preview; they work in the exported PDF. The resolution machinery is in place and activates once typst grows paged anchors.
- **Pathologically nested Markdown can crash the preview** — comrak caps list nesting (`MAX_LIST_DEPTH`) but not blockquote nesting, so AST depth is unbounded. The converter's footnote collection walks iteratively, but the block renderer recurses with the AST depth, so a contrived document (tens of KB of `> ` markers) can overflow the render thread's stack. The proper fix is a general AST depth limit upstream in comrak, alongside the existing list cap.

## 7. References

- Tauri 2: configuration, capabilities, command docs, dialog plugin, testing.
- Typst 0.15: `typst`, `typst-pdf`, `typst-svg`, `typst-as-lib`.
- Typst markup reference: `https://typst.app/docs/reference/`.
- comrak: `https://docs.rs/comrak` (AST node reference).
- Reference projects: `mdxport` (comrak → typst → PDF), `mdcast` (typst-as-lib, typst-html), `typst-preview` (incremental SVG preview architecture).