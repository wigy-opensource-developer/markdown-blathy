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
3. A toolbar offers, on the left, New / Open / Save (in place; Save As… on Mod+Shift+S) Markdown files and a **Document** settings popover (title, author, auto-heading level, language/hyphenation, and the Markdown extensions/flavor); on the right, a **UI theme** selector (light / system / dark — app chrome only, never the document or PDF), the **Page settings** popover (paper size, margins, text alignment and the Typst template — global preferences, applied live) and **Export PDF** (system save dialog). A non-default template (a bundled theme other than light, or a custom file) is marked by a dot on the Page button, with the template's name in the dot's tooltip. Flavor is a per-document setting (front matter), not a page-setup select; the default is Custom (every extension on).
4. The document style is provided by a bundled Typst template (light / dark / obsidian), selectable from the Page settings popover; users may also supply their own `.typ` template. Templates share a helper prelude and are composed on top of it — see `crates/markdown-blathy-core/templates/README.md` for the contract.
5. Compile warnings (e.g. unknown fonts) are surfaced in the UI, not swallowed.
6. A companion CLI, `markdown-blathy`, reuses the same core for scriptable PDF export, front-matter editing, template validation and launching the GUI with a document; the Linux deb/rpm packages ship it next to the GUI ([docs/CLI.md](docs/CLI.md)).
7. Unsaved work is protected: the window title marks edits, closing the window (or New/Open) asks Save/Discard/Cancel, and a dirty document is snapshotted every ~30 s; after a crash the next launch offers the newest snapshot (Restore/Discard) instead of losing it.

## 2. Non-goals (v1)

- No incremental compilation (tinymist-style). Full recompile per change is fine for typical documents (< ~50 pages).
- No client-side Typst WASM (`typst.ts`). All compilation happens in Rust; this avoids version drift between preview and PDF.
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
│  src-tauri — Tauri adapter: thin #[tauri::command] wrappers, system      │
│  dialogs, prewarm hint, startup-document slot                            │
│      ▼                                                                   │
│  crates/markdown-blathy-core — the real logic (no tauri imports):        │
│  service.rs             shared high-level ops (GUI and CLI) + cache      │
│  markdown::convert      comrak → AST → Typst markup (escaped)            │
│  typst_engine::template compose prelude + user template + invocation     │
│  typst_engine::engine   TypstEngine (typst-as-lib) → compile             │
│      ├── typst_svg  → per-page SVG strings  (preview)                    │
│      └── typst_pdf  → PDF bytes             (export)                     │
└──────────────────────────────────────────────────────────────────────────┘
```

**Design rule:** `markdown-blathy-core` must not import `tauri`. The `#[tauri::command]` functions are thin serialization/dialog wrappers. This makes the core unit-testable without a running app, and lets the CLI reuse it.

**Renders are stateless** — each invoke recompiles the document from the current inputs; no document state is kept between invokes. The command layer does hold process-level state: the engine cache (one `TypstEngine` per static source), the consumed-once startup-document slot, and the prewarm hint written after each successful render. Perf is acceptable: Typst compiles a typical page in single-digit milliseconds, and the engine cache + memoization keep per-render cost low.

**The render cycle is single-flight.** The frontend debounces edits by 200 ms and keeps at most one preview compile in flight: edits during a render only mark a catch-up owed, which starts when the render settles and the last edit is ≥200 ms old (`src/lib/renderScheduler.js`). Loads invalidate an in-flight response through the seq guard but still wait for it to settle. Save and close never wait for a render.

**The preview is virtualized.** A render lands as metadata (page count + intrinsic sizes + the template's page fill) plus the visible window's SVGs, so the swap from the last-good pages is atomic — they stay up, dimmed under the delayed "Rendering…" note, until the new ones land. The backend keeps the ONE compiled document; scrolling exports further pages on demand (`export_pages`), dropping their markup when they scroll away. Unexported pages show as placeholder sheets in the template's paper color with a "Rendering…" note. A current/total page badge floats in the preview pane (the topmost visible page, from scroll geometry alone). Layout is still whole-document — Typst page breaks flow across pages — but export, IPC and DOM cost scale with the visible window, not the document.

**The CLI is a second thin adapter.** `crates/markdown-blathy` (binary `markdown-blathy`) wraps the same `Service` for scriptable PDF export, front-matter get/set, template validation and listing, and the `gui` launcher, which hands a file to the GUI through the startup-document slot ([docs/CLI.md](docs/CLI.md)).

## 4. Flavor support

A *flavor* is a named preset of extension flags. The effective behavior is a
set of feature flags; a document can override its flavor and flip individual
extensions via front matter (see below).

| Feature | CommonMark | GitHub | Custom |
|---|---|---|---|
| strikethrough | — | ✅ | ✅ |
| tables | — | ✅ | ✅ |
| task lists | — | ✅ | ✅ |
| footnotes | — | — | ✅ |
| math (`$...$`) | — | — | ✅ |
| highlight (`==text==`) | — | — | ✅ |
| subscript (`~text~`) | — | — | ✅ |
| superscript (`^text^`) | — | — | ✅ |
| description lists | — | — | ✅ |
| page breaks (`<!-- pagebreak -->`) | — | — | ✅ |
| table of contents (`[[_TOC_]]`) | — | — | ✅ |

Heading IDs are always on in every flavor — core app behavior (anchors,
TOC), not an `ext_*` toggle.

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
close (`lib/renderScheduler.js`, `.modal-open` in `styles.css`). The
unsaved-changes prompt uses the same treatment.

**The unsaved-changes prompt guards every destructive request.** Closing the
window, New and Open (including a document the CLI asked to open) first run
through `lib/unsavedGuard.js`: a clean document proceeds at once; a dirty one
(front-matter edits included) asks Save / Discard / Cancel, and choosing Save
resumes the *same* interrupted request only after the save reached disk. A
cancelled Save As or a failed save keeps the prompt open. The window title
carries a `•` while the document is dirty. The backend intercepts
`CloseRequested` and asks the frontend via one `app-request` event
(`close`/`open`), so no dialog blocks the window event loop; `confirm_close`
arms a one-shot allowance for the approved close.

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
| **M8 — Packaging & release** | Per-platform reusable workflows (deb, rpm, AppImage, macOS, Windows) with a tag-triggered draft release, bundle + AppImage metadata, Windows icon | ✅ Done |
| **M9 — Data safety & headless test environment** | Headless build/E2E environment + smoke test, plain save, dirty state + close guard, autosave recovery, export warnings surfaced in the UI | ✅ Done |

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
3. **Per-platform workflows + tag-triggered release.** Each artifact has its
   own workflow, dispatchable from the Actions tab and reusable from the
   release via `workflow_call`:

   | Workflow | Artifact | Runner |
   |---|---|---|
   | `linux-deb.yml` | `.deb` | ubuntu-24.04 |
   | `linux-rpm.yml` | `.rpm` | ubuntu-24.04 |
   | `linux-appimage.yml` | `.AppImage` (via `scripts/build-appimage.py`) | ubuntu-24.04 |
   | `macos.yml` | universal `.dmg` (unsigned unless signing secrets are set) | macos-15 |
   | `windows.yml` | NSIS `.exe` + `.msi` (unsigned unless signing secrets are set) | windows-latest |

   The shared GTK/WebKitGTK package list lives in
   `.github/actions/linux-deps/action.yml`. On `v*` tags, `release.yml` calls
   the five workflows, merges their uploaded artifacts and creates a
   **draft** (unpublished) GitHub release with the assets and release notes
   generated by git-cliff from the tag's commits (pinned version; full-depth
   checkout — `fetch-depth: 0`). It consumes the screenshots committed under
   `docs/screenshots/`; it does not generate them. Publishing is always a
   manual step.

### M9 tasks — data safety & headless test environment

All tasks below are implemented and covered by the quality gate.

1. **Headless build and E2E environment.** Install the GTK/WebKitGTK dev
   stack plus the headless display stack (`xvfb`, `xauth`, `dbus-x11`,
   `webkit2gtk-driver`) in dev containers (apt commands in TESTING.md;
   verified on a root Debian 13 container — WebKitGTK 2.54, full
   `cargo test --workspace` links, smoke test renders under Xvfb), and extend
   the CI `linux-deps` action with the headless extras. Gate the smoke test
   from TESTING.md (`xvfb-run -a dbus-run-session -- env
   WEBKIT_DISABLE_COMPOSITING_MODE=1 timeout 15s …`); it passes only when the
   window is alive **and** the frontend rendered a preview, so a blank
   webview fails. A debug build must enable `tauri/custom-protocol` to embed
   `dist/` (a plain `cargo build` points at the dev server). Unblocks the
   WebDriver E2E plan. Acceptance: `cargo test --workspace` and the smoke
   test pass in a bare container.
2. **Plain Save.** `file.save` (Mod+S) writes the open document to its
   existing path without a dialog; only an untitled document opens the
   "Save As…" dialog. `save_document` writes the backend's remembered
   `PathBuf` (set by open/Save As, cleared by New), so a non-UTF-8 filename
   still saves correctly; `file.saveAs` (Mod+Shift+S) stays available for
   changing the path and keeps the front-matter-title default filename.
   Acceptance: edit an opened file, Mod+S, no dialog, the file on disk
   changes.
3. **Dirty state and close guard.** Track edits since the last save/open;
   closing the window — and New/Open — with unsaved edits asks
   Save/Discard/Cancel; the window title carries a dirty marker.
   Acceptance: closing a dirty window never loses edits silently.
4. **Autosave and crash recovery.** While dirty, snapshot the document
   (markdown + path) to app-data every ~30 s; on startup offer the newest
   snapshot (Restore/Discard) and drop it after a successful save.
   Acceptance: killing the process mid-edit offers recovery on relaunch.
5. **Surface export warnings.** `export_pdf` returns `warnings` in its
   `FileOutcome` and the UI lists them after a successful export (the CLI
   already prints them). Acceptance: a document with a duplicate heading ID
   shows the same warnings after export as in the preview.
6. **Remove the dead "Custom…" margin preset** — asymmetric margins are
   not planned, so the Margins select offers the real presets only.
7. **Reconcile the docs** with the new save behavior (the §6 non-UTF-8
   filename note here, README known limitations).

### Deferred issues

Follow-ups found in manual use, to schedule after M8:

- **Taskbar icon flips to a gear and stays there** — opening the Document
  popover or any Open/Save/Export dialog changes the window/taskbar icon from
  the blue MD icon to a gear icon, and closing the dialog leaves the gear in
  place. Almost certainly a GTK/WebKitGTK window-icon side effect when an
  auxiliary window/portal is mapped; needs a window-level fix (e.g.
  re-asserting the app icon on focus/close). Cosmetic, no functional impact.
- **Single-instance forwarding** — every `markdown-blathy gui` launch starts
  a new instance (docs/CLI.md); forwarding to a running instance is not
  implemented. The `app-request` `open` event and the dirty guard are in
  place, so the forwarding task only needs to deliver the path to the running
  backend and emit that event.
- **Native menu entries** — the command registry (`src/lib/commands.js`) is
  ready for them, but no menu bar is wired; most visible on macOS.
- **E2E WebDriver suite** — the plan (harness, five journeys, CI gating) is
  in TESTING.md; nothing is implemented yet. The headless environment it
  needs lands with M9 task 1.

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
- **Code-block highlighting follows typst's built-in grammars** — fenced blocks with a recognized language tag are highlighted (typst embeds syntect/two-face); unknown tags render unstyled, and there is no highlight-theme setting.
- Only the embedded DejaVu family by default (Sans regular + bold, Sans Mono for code, Math TeX Gyre for math); no CJK coverage out of the box. Custom templates can reference system fonts and ship font files next to the `.typ` file.
- **System-font naming is typst's, not fontconfig's** — typst strips styling suffixes from family names, so a font whose *name* carries the styling may be unreachable. Use the trimmed family name or a font without a styling suffix.
- Remote images unsupported; images must be local files reachable through the world's file resolver.
- **Math is LaTeX-flavored, but not a full LaTeX engine.** Common LaTeX math (scripts, fractions/roots, symbols, styles, matrices, common macros) is translated to Typst math. Packages, exotic TeX primitives and some environment/TikZ edge cases remain unsupported; translator diagnostics report them.
- No incremental compilation — full recompile per change.
- **Linux UI text is grayscale-antialiased** — WebKitGTK renders text onto composited transparent layers. The Typst SVG preview is vector paths and unaffected.
- **Non-UTF-8 filenames on legacy-locale Linux** — paths travel over IPC as JSON strings (UTF-8), so a filename in e.g. latin-2 bytes is echoed back lossily: the title bar and dialog defaults may show replacement characters. File *contents* load, save-in-place and crash recovery use the backend's exact `PathBuf` (and the snapshot's exact path bytes), so writes always go to the correct file.
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