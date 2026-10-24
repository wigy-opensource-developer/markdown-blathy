# MarkdownBláthy — Product Specification

| | |
|---|---|
| **Status** | v1.0 |
| **Target** | Desktop app (Windows / macOS / Linux) built with Tauri 2 + Rust + Typst |
| **Reader** | Implementer (AI agent or human). Assumes solid Rust & Tokio; beginner–intermediate Tauri 2 and HTML/CSS. |

> **Note:** This is an implementation specification — it describes how the app is built, not how to use it. For a user-facing overview see [README.md](README.md).
>
> **Motto:** Markdown, transformed.

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
3. A toolbar offers, on the left, New / Open / Save Markdown files and a **Document** settings popover (title, author, auto-heading level, language/hyphenation, and the Markdown extensions/flavor); on the right, a **Page settings** popover (paper size, margins, text alignment and the Typst template — global preferences, applied live) and **Export PDF** (system save dialog). A custom template is marked by a dot on the Page button. Flavor is a per-document setting (front matter), not a page-setup select; the default is Custom (every extension on).
4. The document style is provided by a bundled Typst template (light / dark / obsidian), selectable from the Page settings popover; users may also supply their own `.typ` template. Templates share a helper prelude and are composed on top of it — see `src-tauri/templates/README.md` for the contract.
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
│  Mantine toolbar / status bar / error banner                             │
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

**Language and hyphenation.** `lang: <ISO 639>` (two or three ASCII letters,
e.g. `hu`) sets the Typst text language for the whole document, which selects
the hyphenation patterns — without it Typst's default is English, so non-English
documents get English break points. `hyphenate: true|false` overrides
hyphenation explicitly (`auto`, the default, hyphenates only justified text).
Both become a `#set text(…)` rule at the top of the converted body. Invalid
values warn and are dropped before Typst sees them. The Document settings
popover edits both (language field + auto/on/off segmented control); clearing
writes no key. There is no automatic language detection — English and auto
hyphenation remain the defaults.

**The Document popover is modal.** While it is open the editor, preview and
status bar are blurred, `inert` and frozen: a render scheduled by earlier
editor keystrokes is cancelled on open, a render that finishes meanwhile is
stashed, and the owed/stashed render is shown in a single catch-up pass on
close (`lib/previewFreeze.js`, `.settings-open` in `styles.css`).

**The settings draft is uncontrolled.** The form reads its values from the
DOM on Apply (diffed against the mount-time snapshot), and the segmented
control, checkboxes and presets never call `setState` either. This is a
performance requirement, not a style: any React commit while the popover is
open makes WebKitGTK run a full-document pass over the rendered preview —
measured at ~800 ms per keystroke against a 32-page preview, versus ~40 ms
with the draft uncontrolled. A preview render itself is far too slow to run
under the user's hands, so it stays frozen until close.

**Extension overrides.** On top of the `flavor:` preset, each extension can be
toggled with an `ext_<name>: true|false` key (e.g. `ext_footnotes: true`).
When the app writes front matter it uses the XOR-minimal encoding: the preset
with the fewest flips, plus only the differing flags.

**Math input is LaTeX-flavored.** Markdown authors write inline and display
math in TeX/LaTeX notation (`$...$`, `$$...$$`) — the convention used by
pandoc, GitHub, Obsidian, Jupyter and most Markdown tooling. Typst is the
rendering backend, not the document-facing math dialect. A dedicated
translator (`markdown/math.rs`) converts LaTeX math to Typst math; raw math is
**not** passed through. It covers common notation: scripts (including TeX's
empty base, `$^+$`), fractions, roots, sums/integrals, Greek letters and
operators, text/style wrappers, matrices, delimiters, `\stackrel`, and common
macro definitions. Translator warnings and invalid output become math
diagnostics with the Markdown source line and, where available, a suggested
fix. This is not a full LaTeX engine: packages, exotic primitives and some
TikZ/environment edge cases remain unsupported; those are reported rather than
silently mangled. Math remains opt-in (Custom flavor / `ext_math`).

**Explicit bare links** (`<https://example.com>`) are supported in **all flavors** — this is standard CommonMark, not an extension. The `autolink` extension (automatic bare-URL linking) is **off in every flavor**.

**Long unbreakable tokens wrap.** Typst's line breaker follows Unicode
UAX #14, so a long run with no break opportunity (a chemical formula, a hash)
would cross the margin — most visibly in a table cell. The converter hands
tokens of 24+ characters to the prelude's `break-long`, which inserts
zero-width `box` break opportunities: the token wraps where it must, the boxes
are invisible, and they stay out of the PDF text layer (search and copy are
unaffected). A template may override `break-long` (e.g. `#let break-long(s) = s`)
to turn this off.

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
| **M7 — Logging & extended syntax** | `tracing` + SVG internal links + heading IDs + new constructs | ✅ Done |
| **M8 — Packaging & release** | Tag-triggered .deb/.rpm/AppImage builds with a draft release, bundle + AppImage metadata, Windows icon | ✅ Done |

See **IMPLEMENTATION.md** for acceptance criteria and test plans per milestone.

### M8 tasks (in-repo, in order)

1. **Windows icon.** Add `src-tauri/icons/icon.ico` and list it in
   `bundle.icon` so Windows builds can bundle (macOS/Windows are secondary
   targets, but the icon belongs in the bundle before the release workflow
   covers them).
2. **Bundle and AppImage metadata.** Fill in the Tauri bundle metadata
   (`description`, `category`, `longDescription`, …) that .deb/.rpm read,
   and add an AppStream metainfo file — name, summary, description,
   keywords (tags), screenshots, releases — copied into the AppDir at
   `usr/share/metainfo/`. This is what `appimagetool` asks for when it warns
   about missing metadata.

   Screenshots are hosted URLs, not package files: commit them under
   `docs/screenshots/` and reference them by direct raw link pinned to the
   release tag —
   `https://raw.githubusercontent.com/<owner>/<repo>/v<version>/docs/screenshots/<name>.png`.
   No custom domain or GitHub Pages is needed, and a tag name is known before
   the release commit, so the metainfo can point at its own tag; earlier
   releases keep resolving their pinned images. `appstreamcli` fetches these
   URLs during validation (`AS_VALIDATE_NONET=1` suppresses that for local
   pre-tag builds). Regenerating the images is a manual pre-release step on
   the developer machine — CI never creates them (tools and procedure:
   CONTRIBUTING.md, How to release a new version).
3. **Tag-triggered release workflow.** On `v*` tags: build the production
   binary, package `.deb`, `.rpm` and the AppImage, and create a **draft**
   (unpublished) GitHub release with the artifacts attached and release
   notes generated by git-cliff from the tag's commits (pinned version;
   full-depth checkout — `fetch-depth: 0`). It consumes the screenshots
   committed under `docs/screenshots/`; it does not generate them.
   Publishing is always a manual step. The existing `appimage.yml` stays as
   the manual-dispatch fallback.

### Deferred issues

Follow-ups found in manual use, to schedule after M8:

- **Taskbar icon flips to a gear and stays there** — opening the Document
  popover or any Open/Save/Export dialog changes the window/taskbar icon from
  the blue MD icon to a gear icon, and closing the dialog leaves the gear in
  place. Almost certainly a GTK/WebKitGTK window-icon side effect when an
  auxiliary window/portal is mapped; needs a window-level fix (e.g.
  re-asserting the app icon on focus/close). Cosmetic, no functional impact.

### Upstream dependencies (patch required outside this repo)

The two remaining §6 limitations are rooted in upstream crates, not in this
codebase. They are listed last so the in-repo M8 work can finish first; each
one closes only when the upstream crate is patched — there is no in-repo fix.

- **comrak — AST depth limit.** comrak caps list nesting
  (`MAX_LIST_DEPTH = 100`) but not blockquote nesting, so a contrived
  document can still overflow the converter's block renderer. The proper fix
  is a general AST depth limit upstream in comrak, next to the list cap. The
  in-repo traversals that can be iterative already are (footnotes, headings,
  inline content); the block renderer keeps recursing until comrak caps
  depth.
- **typst — paged anchors.** typst 0.15's paged introspector implements
  `anchor()` as `None` (anchors exist only in the HTML export path), so SVG
  internal links emit `<a>` without `href`. The resolution machinery
  (`LateLinkResolver` + `svg_in_bundle`) is already in place and activates
  once typst grows paged anchors; PDF links work today.

## 6. Limitations

- Raw HTML in Markdown is stripped to text, except the `<!-- pagebreak -->` marker (Custom flavor), which becomes a page break.
- No syntax highlighting inside code blocks in the PDF.
- Only the embedded DejaVu family by default (Sans regular + bold, Sans Mono for code, Math TeX Gyre for math); no CJK coverage out of the box. Custom templates can reference system fonts and ship font files next to the `.typ` file.
- **System-font naming is typst's, not fontconfig's** — typst strips styling suffixes from family names, so a font whose *name* carries the styling may be unreachable. Use the trimmed family name or a font without a styling suffix.
- Remote images unsupported; images must be local files reachable through the world's file resolver.
- **Math is LaTeX-flavored, but not a full LaTeX engine.** Common LaTeX math (scripts, fractions/roots, symbols, styles, matrices, common macros) is translated to Typst math. Packages, exotic TeX primitives and some environment/TikZ edge cases remain unsupported; translator diagnostics report them.
- No incremental compilation — full recompile per change.
- **Linux UI text is grayscale-antialiased** — WebKitGTK renders text onto composited transparent layers. The Typst SVG preview is vector paths and unaffected.
- **Non-UTF-8 filenames on legacy-locale Linux** — paths travel over IPC as JSON strings (UTF-8), so a filename in e.g. latin-2 bytes is echoed back lossily. File *contents* still load and writes still go to the correct file.
- SVG preview text is not selectable (inline SVG).
- **Emoji shortcodes** (`:smile:`) are not expanded — the author pastes actual emoji characters.
- **Automatic URL linking** is not enabled — users write explicit bare links (`<https://example.com>`) when they want a link.
- **Table of contents**: the `[[_TOC_]]` marker (GitLab convention) triggers an auto-generated table of contents in Custom flavor.
- **Internal links in the SVG preview are inert on typst 0.15** — the paged introspector has no anchors, so footnote/heading/TOC links carry no `href` in the preview; they work in the exported PDF. The resolution machinery is in place and activates once typst grows paged anchors (see §5, Upstream dependencies — patch required upstream).
- **Pathologically nested Markdown can crash the preview** — comrak caps list nesting (`MAX_LIST_DEPTH`) but not blockquote nesting, so AST depth is unbounded. The converter's footnote collection walks iteratively, but the block renderer recurses with the AST depth, so a contrived document (tens of KB of `> ` markers) can overflow the render thread's stack. The proper fix is a general AST depth limit upstream in comrak, alongside the existing list cap (see §5, Upstream dependencies — patch required upstream).

## 7. References

- Tauri 2: configuration, capabilities, command docs, dialog plugin, testing.
- Typst 0.15: `typst`, `typst-pdf`, `typst-svg`, `typst-as-lib`.
- Typst markup reference: `https://typst.app/docs/reference/`.
- comrak: `https://docs.rs/comrak` (AST node reference).
- Reference projects: `mdxport` (comrak → typst → PDF), `mdcast` (typst-as-lib, typst-html), `typst-preview` (incremental SVG preview architecture).