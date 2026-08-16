# Tauri MD Editor — Test Plan

## Test matrix

### `markdown/convert.rs` (unit, table-driven)

| Test | Type | Priority |
|---|---|---|
| escapes hash/brackets/asterisks/underscores/dollar/backslash in text | unit | high |
| maps heading levels to `=`, `==`, `===`, … | unit | high |
| maps bold/italic/strikethrough | unit | high |
| maps inline code and code blocks (with language, without language, containing backticks) | unit | high |
| maps links with and without label, URL containing quotes | unit | high |
| maps ordered and unordered lists, nested lists | unit | high |
| maps task list items to `#task-list(task-done([…]), task-todo([…]), task-plain([…]))` | unit | high |
| maps GFM table to `#markdown-table(header: …, rows: …)` (GFM/Custom only) | unit | high |
| maps footnotes and math in Custom flavor; leaves them as text otherwise | unit | high |
| maps heading IDs: `{#custom-id}` parsed from header_attributes; auto-slug = kebab-case of plain text; duplicate slugs → warning in `ConvertedDocument.warnings`, first wins | unit | high |
| maps explicit bare links `<url>` as Link with label = URL text | unit | high |
| maps highlight, subscript, superscript in Custom flavor; leaves as text in CommonMark/GFM | unit | high |
| maps definition lists in Custom flavor; leaves as text otherwise | unit | high |
| strips HTML tags and keeps text (lossy path) | unit | high |
| parses YAML front matter title/author; ignores other keys | unit | high |
| empty input returns `[]` | unit | high |

### `typst_engine/template.rs` (unit)

| Test | Type | Priority |
|---|---|---|
| compose emits template + article invocation with expected args | unit | high |
| build_static_source maps paper/margins/alignment to template args | unit | high |
| render_inputs forwards front-matter title/author and the converted body; returns `(Dict, Vec<String>)` | unit | high |
| prelude contract: default template compiles a probe document; user templates are composed after the prelude | unit | high |
| probe_template: accepts a styling-only template (defaults apply), accepts valid helper overrides, rejects syntax errors and broken overrides | unit | high |
| probe exercises all converter helpers including mark, superscript, subscript, definition-list, and markdown-toc | unit | high |

### `typst_engine/engine.rs` (integration, slow — mark with `#[ignore]`)

| Test | Type | Priority |
|---|---|---|
| compile_preview returns at least one SVG containing `<svg` | integration | high |
| compile_pdf bytes start with `%PDF` and are non-empty | integration | high |
| compile error path yields diagnostics, not a panic | integration | high |
| custom template with a broken override fails the probe compile | integration | high |
| warnings are collected not fatal (unknown font → warning, not error) | integration | high |
| system fonts are used when enabled (real installed family renders warning-free with `FontOptions::system_fonts`, warns without) | integration | medium |
| compile_preview_pages resolves internal links: SVG contains `href="#` for footnote markers and heading anchors, and matching `id="..."` target points | integration | high |
| svg_in_bundle with LateLinkResolver + anchors produces clickable internal links (verified by inspecting SVG output for `href="#..."` and `id="..."`) | integration | high |
| duplicate heading IDs produce a warning in `ConvertedDocument.warnings`; only first heading carries the label | integration | high |
| heading slug = kebab-case(plain text) for all flavors; custom `{#id}` overrides slug | integration | high |
| highlight/subscript/superscript map to `#mark`/`#subscript`/`#superscript` in Custom flavor; not mapped in CommonMark/GFM | integration | high |
| definition list maps to `#definition-list((term, [def]), …)` in Custom flavor | integration | high |

### Frontend (Vitest, pure modules)

| Test | Module | Priority |
|---|---|---|
| loadSettings returns defaults when nothing is stored | settings.js | high |
| loadSettings round-trips: settings saved for one session load on the next | settings.js | high |
| loadSettings falls back to defaults on corrupt JSON | settings.js | high |
| loadSettings merges a partial stored options object onto the defaults | settings.js | high |
| loadSettings falls back to default margins when stored margins are incomplete | settings.js | high |
| saveSettings writes JSON under the tauri-md-editor.settings key | settings.js | high |
| applyOptionsPatch ignores null patches for fields that cannot be empty (Select deselect) | settings.js | high |
| applyOptionsPatch keeps null templatePath (clearing custom template is legitimate) | settings.js | high |
| applyOptionsPatch applies real values and leaves unpatched fields untouched | settings.js | high |
| isStaleResponse correctly identifies stale vs fresh sequence numbers | seq.js | high |
| api wrappers call invoke with correct command names | api.js | high |
| FLAVORS, PAPERS, MARGIN_PRESETS, ALIGNS have expected values | presets.js | high |

Component tests (React Testing Library) are not required for v1; the seq guard and debounce are simple enough to review by hand.

## Milestone acceptance criteria

### M0 — Scaffold
- [ ] `npm run tauri dev` opens an empty Mantine window
- [ ] `cargo test` runs (0 tests)
- [ ] `./check.sh` passes (build + fmt + clippy)

### M1 — Converter
- [ ] 27 unit tests green
- [ ] CommonMark/GFM/Custom flavour differences verified by tests

### M2 — Engine
- [ ] `compile_preview(sample)` returns ≥1 SVG containing `<svg`
- [ ] `compile_pdf` produces bytes starting with `%PDF`
- [ ] Error path yields `line N: …` diagnostics
- [ ] Warnings are collected, not fatal

### M3 — UI loop
- [ ] CodeMirror 6 editor, Typst SVG preview, 200 ms debounce + seq guard in App.jsx
- [ ] `render_preview` Tauri command registered
- [ ] Vitest suite for lib/presets.js + lib/seq.js (14 frontend + 45 Rust tests green via `./check.sh`)

### M4 — Files & export
- [ ] `open_markdown`/`save_markdown`/`export_pdf`/`get_initial_directory`/`open_external` registered in lib.rs
- [ ] Writes are atomic (`files.rs::write_atomically`)
- [ ] Dialogs via `tauri-plugin-dialog` start in the working Directory and default to `<title>.md`/`<title>.pdf`
- [ ] Exported PDF opens in the system viewer via the `open` crate
- [ ] Task lists render as a checkbox grid and footnote references are inlined at their call site
- [ ] Preview links go to the system browser, other schemes are rejected, fragment links stay internal
- [ ] Canceled dialogs are silent
- [ ] 66 Rust + 47 frontend tests green via `./check.sh`

### M5 — Settings & polish
- [ ] Settings persist to localStorage (loaded on startup, saved on every option change, merged over defaults on corrupt/partial data)
- [ ] Template… button picks a `.typ` file via dialog plugin and applies only after probe validation
- [ ] Template system is a prelude: default.typ provides default implementations of all converter helpers; user templates composed after it so Typst shadowing gives "default unless redefined"
- [ ] Active custom template shown in toolbar
- [ ] Warnings render as a collapsible list in the status bar
- [ ] Empty doc renders exactly one blank page
- [ ] 73 Rust + 54 frontend tests green via `./check.sh`

### M6 — Engine cache & fonts
- [ ] Pipeline split into static source (template + toolbar options + `#article(...)` invocation reading `title`/`author`/`body` from `sys.inputs`) and per-render inputs
- [ ] `TemplateEngineCache` holds one engine per static source, rebuilds only when template or toolbar options change
- [ ] Custom templates load system fonts (`typst-kit-fonts` feature) plus font files next to the `.typ` file
- [ ] `figure-image` added to prelude
- [ ] 77 Rust + 54 frontend tests green via `./check.sh`

### M7 — Logging & extended syntax
- [ ] `tracing` + `EnvFilter` replaces `eprintln!`; `RUST_LOG=debug` shows cache rebuilds and preview timing
- [ ] SVG internal link resolution via `svg_in_bundle` with `LateLinkResolver` + anchors
- [ ] Heading IDs: auto slug = kebab-case(plain text); `{#custom-id}` overrides; duplicate warning
- [ ] Explicit bare links (`<url>`) supported in all flavors
- [ ] Highlight/subscript/superscript/definition lists in Custom flavor
- [ ] TOC: `[[_TOC_]]` marker triggers `#markdown-toc()` in Custom flavor
- [ ] Probe exercises all new helpers
- [ ] Manual verification: open document with footnote → preview shows clickable marker → click scrolls to note

## Test quality rules

- Follow red-green-refactor. One failing test at a time.
- `markdown/` and `typst_engine/` are plain Rust (no `tauri` imports) — tests run with `cargo test`.
- Frontend pure modules (`lib/*.js`) testable with Vitest.
- Component tests not required for v1.
- Slow engine tests marked with `#[ignore]`.
- No untested production code.