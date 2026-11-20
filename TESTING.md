# MarkdownBláthy — Test Plan

## Test matrix

### `markdown/convert.rs` (unit, table-driven)

| Test | Type | Priority |
|---|---|---|
| escapes hash/brackets/asterisks/underscores/dollar/backslash in text | unit | high |
| escapes Typst label delimiters (`<`/`>`), references (`@`), raw marker (`` ` ``) and non-breaking space (`~`); escapes `/` at line start (term list) | unit | high |
| maps heading levels to `=`, `==`, `===`, … | unit | high |
| maps bold/italic/strikethrough | unit | high |
| maps inline code and code blocks (with language, without language, containing backticks) | unit | high |
| maps links with and without label, URL containing quotes | unit | high |
| maps ordered and unordered lists, nested lists | unit | high |
| maps task list items to `#task-list(task-done([…]), task-todo([…]), task-plain([…]))` | unit | high |
| maps GFM table to `#markdown-table(header: …, rows: …)` (GFM/Custom only) | unit | high |
| maps footnotes and math in Custom flavor; leaves them as text otherwise | unit | high |
| `math::translate`: fractions/roots/scripts, styles/text, multi-letter variables, `\stackrel`, matrices/delimiters; clean supported input has no diagnostics | unit | high |
| `math::translate`: translator warnings become diagnostics; empty script base has a suggested fix; translated Typst is syntax-valid | unit | high |
| converter surfaces math diagnostics with the Markdown source line and fix text | unit | high |
| maps heading IDs: manual `{#custom-id}` always honored (every level, any `maxLevel`); auto-slug = kebab-case of plain text, gated by `maxLevel`; one uniqueness namespace, duplicates → warning, first wins; emitted as `#label("id")` | unit | high |
| maps internal links `[x](#slug)` to `#internal-link("slug")[x]`; external URLs keep `#link("url")`; empty fragment stays a URL link | unit | high |
| maps explicit bare links `<url>` as Link with label = URL text | unit | high |
| maps highlight, subscript, superscript in Custom flavor; leaves as text in CommonMark/GFM | unit | high |
| maps definition lists in Custom flavor; leaves as text otherwise | unit | high |
| strips HTML tags and keeps text (lossy path) | unit | high |
| parses YAML front matter title/author + `headingIdsMaxLevel` override (invalid value warns, keeps toolbar setting); ignores other keys | unit | high |
| empty input returns `[]` | unit | high |
| source map emits one anchor per leaf-block start line (first wins), none for outer containers or relocated footnote definitions | unit | high |

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
| literal Typst markup characters in prose — `(<5–15%)`, an email address, a stray backtick, `~5`, a line-start `/` — compile through the full pipeline (regression for "unclosed label") | integration | high |
| compile_pdf bytes start with `%PDF` and are non-empty | integration | high |
| compile error path yields diagnostics, not a panic | integration | high |
| custom template with a broken override fails the probe compile | integration | high |
| warnings are collected not fatal (unknown font → warning, not error) | integration | high |
| system fonts are used when enabled (real installed family renders warning-free with `FontOptions::system_fonts`, warns without) | integration | medium |
| compile_preview_pages resolves internal links: SVG contains `href="#` for footnote markers and heading anchors, and matching `id="..."` target points — **`#[ignore]`d**: typst 0.15 paged introspector has no anchors, so SVG Location links are unresolved (PDF links work); un-ignore when typst grows paged anchors | integration | high |
| svg_in_bundle with LateLinkResolver + anchors produces clickable internal links (verified by inspecting SVG output for `href="#..."` and `id="..."`) — **`#[ignore]`d**, same typst 0.15 limitation | integration | high |
| heading labels render invisibly (same glyph count with/without ID) and are real link targets (`#link(label(...))` to the converted body compiles) | integration | high |
| internal links: `[go](#title)` renders a link element; dangling `[go](#missing)` degrades to plain text without failing the compile | integration | high |
| converter warnings (duplicate heading IDs, invalid front-matter values) reach the command layer and are merged ahead of engine warnings | unit | high |
| duplicate heading IDs produce a warning in `ConvertedDocument.warnings`; only first heading carries the label | integration | high |
| heading slug = kebab-case(plain text) for all flavors; custom `{#id}` overrides slug | integration | high |
| highlight/subscript/superscript map to `#mark`/`#subscript`/`#superscript` in Custom flavor; not mapped in CommonMark/GFM | integration | high |
| definition list maps to `#definition-list((term, [def]), …)` in Custom flavor | integration | high |
| long unbreakable tokens are wrapped in the prelude's `break-long`; short words are untouched | unit | high |
| a long chemical formula in a table cell wraps inside the margin; the control (with `break-long` disabled) overflows | integration | high |
| TOC entries keep the rendered heading markup without double escaping (`_Bold_`, not literal backslashes) | unit | medium |
| source metadata resolves to monotonic leaf-block positions and changes neither page sizes nor SVG bytes (including tight lists) | integration | high |

### `recovery.rs` (unit, M9)

| Test | Type | Priority |
|---|---|---|
| save → take offers the snapshot for a missing file; identical-to-disk and stale slots are pruned | unit | high |
| untitled snapshots are always offered; the newest of several slots wins | unit | high |
| restore preserves the exact path bytes and keeps the snapshot for a second crash | unit | high |
| discard removes one slot; clear removes the document and untitled slots | unit | high |
| ids from the frontend cannot escape the recovery directory (traversal/invalid shapes) | unit | high |

### Frontend (Vitest, pure modules)

| Test | Module | Priority |
|---|---|---|
| loadSettings returns defaults when nothing is stored | settings.js | high |
| loadSettings round-trips: settings saved for one session load on the next | settings.js | high |
| loadSettings falls back to defaults on corrupt JSON | settings.js | high |
| loadSettings merges a partial stored options object onto the defaults | settings.js | high |
| loadSettings falls back to default margins when stored margins are incomplete | settings.js | high |
| saveSettings writes JSON under the markdown-blathy.settings key | settings.js | high |
| applyOptionsPatch ignores null patches for fields that cannot be empty (Select deselect) | settings.js | high |
| applyOptionsPatch keeps null templatePath (clearing custom template is legitimate) | settings.js | high |
| applyOptionsPatch applies real values and leaves unpatched fields untouched | settings.js | high |
| isStaleResponse correctly identifies stale vs fresh sequence numbers | seq.js | high |
| api wrappers call invoke with correct command names | api.js | high |
| FLAVORS, PAPERS, MARGIN_PRESETS, ALIGNS have expected values | presets.js | high |
| Document patch diffs the form; hyphenate is tri-state (absent = keep, null = clear, boolean = set); feature maps compare order-insensitively | docSettings.js | high |
| Render scheduler debounces edits (200 ms), keeps one compile in flight, catches up once after it settles (only for edits ≥200 ms old), and freezes/stashes around modals | renderScheduler.js | high |
| Virtualized-preview geometry: sheets scale to the pane keeping aspect ratio, total height known from metadata, visible range ± overscan clamped, current page = topmost visible (null when unmeasured) | previewWindow.js | high |
| Scroll mapping is linear per segment and invertible, includes document boundaries, skips duplicate/non-monotonic anchors, pauses for stale/disabled maps, never moves keyboard focus; programmatic writes cause no feedback | scrollSync.js | high |
| Keep-all page cache: every page exported for the seq held (instant scroll-back, no refetch), render-seeded window held, fetches deduplicated top-first, new seq resets, stale responses dropped | previewPages.js | high |
| export_pages exports indexed PNG rasters (base64) + link rects from the single cached render at a target pixel width; a seq mismatch serializes stale:true | commands.rs | high |
| export_pages_png renders valid PNG bytes (magic) of the paged document and emits external-link rectangles by recursing group subframes | engine.rs | high |
| page_background resolves the template's page fill for placeholder sheets (auto → white, solid → hex, transparent/gradient → None) | engine.rs | medium |
| Placeholder sheets wear the template paper color and a tilted aria-hidden "Rendering…" note; mounted sheets show the PNG raster as a data-URL img with scaled link overlays, and stay mounted so scroll-back never re-decodes | Preview.jsx (Sheet) | medium |
| Dirty guard resumes the interrupted Close/New/Open only after a successful save; cancel/failed save keeps the request pending | unsavedGuard.js | high |
| Recovery is offered only to a pristine editor, and only for the opened document's own snapshot | recovery.js | high |
| Export warnings name the PDF, count/list the warnings and offer an explicit dismiss | ExportWarnings.jsx | medium |
| stripWoffFallbacks drops WOFF-1.0 `src` alternatives (any order, quotes) and keeps woff2 | scripts/woff2-only.mjs | medium |

Build guard: `npm run check:bundle` (in `./check.sh`, after the build) fails when
`dist/` contains a `.woff` file or a dangling `.woff` CSS reference.

Component tests render through `react-dom/server`; interaction, focus and
browser layout are intentionally left to the E2E layer described below.

## Headless test environment

The Vitest and Rust suites are headless: they need no X server, but the Rust
binary still links against Tauri's native Linux stack. On Debian/Ubuntu this
is the complete build + test set used by CI:

```bash
sudo apt-get install -y --no-install-recommends \
  libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libdbus-1-dev pkg-config build-essential libssl-dev
```

Without those headers, `cargo clippy`/`cargo test` fail while *linking* the
test binary (`unable to find library -lgdk-3`, `-lwebkit2gtk-4.1`, …).

For a headless display (release smoke test or WebDriver), also install:

```bash
sudo apt-get install -y --no-install-recommends \
  xvfb xauth dbus-x11 webkit2gtk-driver
```

Both lists were verified on a root Debian 13 (trixie) container
(WebKitGTK 2.54, GTK 3.24): after the install the full
`cargo test --workspace` links, `WebKitWebDriver` is on `PATH`, and the
smoke test below renders a preview under Xvfb.

Build the production binary and smoke-test that the window initializes and
the frontend actually renders:

```bash
npm run tauri build -- --no-bundle
./scripts/smoke-headless.sh                # target/release/MarkdownBláthy, 15 s

# Faster debug loop; `custom-protocol` embeds dist/ (a plain cargo build
# points at the Vite dev server and shows a blank window):
npm run build
cargo build -p markdown-blathy-tauri --features tauri/custom-protocol
./scripts/smoke-headless.sh target/debug/markdown-blathy-tauri 15
```

The script runs the binary under `xvfb-run -a dbus-run-session --` with
`WEBKIT_DISABLE_COMPOSITING_MODE=1` and a private `XDG_RUNTIME_DIR`. It
passes only when both hold: the app is still alive at the deadline
(`timeout` exit status 124) **and** the command layer logged `preview
rendered`, which proves the embedded frontend loaded, IPC works and Typst
compiled. A blank webview also stays alive, so the render check is what keeps
the gate honest. Failures print the captured output. AT-SPI and EGL warnings
about the missing accessibility bus or software rendering are expected in a
bare container and do not fail the test.

The CI `headless-smoke` job runs the same script against the
`custom-protocol` debug build on every push and PR, so a GUI that cannot
start — or renders nothing — on a headless Linux runner is caught before
release.

## Completing the UI test framework

The current layers are:

| Layer | Tool | Coverage |
|---|---|---|
| Converters/engine | `cargo test` | Markdown → Typst, compile, fonts, cache, commands |
| Pure frontend logic | Vitest | settings, titles, presets, seq guard, freeze/status |
| Components | Vitest + `react-dom/server` | static markup contracts |
| Build guards | Node scripts | bundle budget, WOFF, licenses, CSP |

What is missing is a **real-webview end-to-end suite**. The unit and component
tests cannot catch integration regressions in the Tauri window: IPC wiring,
CodeMirror input, SVG mounting, popover focus/freeze behavior, or the actual
window title. To complete the framework, add a WebDriver layer around the
production binary.

### 1. Install a driver

Two supported paths:

- **WebdriverIO + `@wdio/tauri-service`** (recommended; it can use the embedded
  provider through `tauri-plugin-wdio-webdriver`, or `tauri-driver`).
- **`tauri-driver` + WebKitWebDriver** on Linux:

  ```bash
  cargo install tauri-driver --locked
  # WebKitWebDriver comes from the webkit2gtk-driver package above.
  ```

Run everything on a virtual display; no physical desktop is required:

```bash
xvfb-run -a dbus-run-session -- npm run test:e2e
```

### 2. Add the harness

Suggested layout:

```text
e2e/
├── wdio.conf.js       # binary path, Xvfb-friendly capabilities, timeouts
└── specs/
    └── smoke.e2e.js   # user journeys, not converter re-tests
```

Add a `test:e2e` npm script that builds/uses
`target/release/MarkdownBláthy` and fails if the binary is absent.
Use stable `data-testid` selectors, wait for observable state (SVG page count,
window title, status text) rather than fixed sleeps, and reset localStorage
between specs.

Minimum critical-path coverage:

1. App launches, shows the sample document, and the window title is
   `Sample - MarkdownBláthy`.
2. Typing Markdown updates the SVG preview after the debounce; a malformed
   document shows diagnostics without dropping the last good preview.
3. The Document popover applies front-matter settings, freezes rendering
   while open, and catches up exactly once on close.
4. Page settings persist across relaunch under `markdown-blathy.settings`.
5. Toolbar actions remain reachable at the minimum window size.

Keep native open/save/export dialogs out of E2E: they are OS-owned and flaky
headlessly. Their command layer is already covered by Rust tests; the E2E
suite should cover in-window flows only.

### 3. Gate it in CI

Add a separate job (not necessarily on every push; nightly or manual is
acceptable if runtime is a concern) that:

1. installs the native build packages above plus `xvfb`, `xauth`,
   `dbus-x11` and `webkit2gtk-driver` (the `headless` input of the
   `linux-deps` action does this);
2. runs `./check.sh`;
3. builds with `npm run tauri build -- --no-bundle`;
4. runs the WebDriver suite under `xvfb-run -a dbus-run-session --`;
5. uploads logs and failure screenshots as workflow artifacts.

Step 1 and the display plumbing already exist: the `headless-smoke` CI job
uses the same action input and runs `scripts/smoke-headless.sh`, which
implements the Xvfb invocation the WebDriver suite will reuse.

A complete framework is reached when the E2E job is required for release
tags, covers the five journeys above, and fails deterministically on
regressions without duplicating the converter or engine test matrix.

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
- [ ] SVG internal link resolution via `svg_in_bundle` with `LateLinkResolver` + anchors — machinery in place; inert in SVG on typst 0.15 (no paged anchors), PDF links work; href test `#[ignore]`d until typst catches up
- [ ] Heading IDs: manual `{#custom-id}` always honored; auto slug = kebab-case(plain text) gated by `maxLevel` (toolbar setting, overridable per doc via front-matter `autoHeadingLevel`); one uniqueness namespace, duplicate warning; emitted as invisible `#label("id")`
- [ ] Internal links: `[x](#slug)` → prelude `internal-link` (works in PDF; broken targets degrade to plain text)
- [ ] Explicit bare links (`<url>`) supported in all flavors
- [ ] Highlight/subscript/superscript/definition lists in Custom flavor
- [ ] TOC: `[[_TOC_]]` marker triggers `#markdown-toc()` in Custom flavor
- [ ] Probe exercises all new helpers
- [ ] Manual verification: open a document with a footnote → the preview shows the marker and note (links are inert in the SVG on typst 0.15; they work in the exported PDF)

### M9 — Data safety & headless environment
- [x] GTK/WebKitGTK + Xvfb/dbus/WebKitWebDriver install documented and verified in a bare container; `cargo test --workspace` links
- [x] `scripts/smoke-headless.sh` runs the GUI under Xvfb and requires both a live window and a rendered preview; the `headless-smoke` CI job gates every push/PR
- [x] Mod+S saves in place through the backend's exact `PathBuf`; Mod+Shift+S is Save As and keeps the title-based default filename; New clears the path
- [x] Dirty marker in the window title; Close/New/Open (including CLI opens) ask Save/Discard/Cancel; a successful save resumes the interrupted request
- [x] Recovery snapshots every 30 s while dirty, one atomic slot per document (exact path bytes); startup offers Restore/Discard only to a pristine editor; snapshots survive until discard or a successful save
- [x] Export returns compile warnings and the UI shows them in a dismissible panel
- [x] The dead "Custom…" margin preset is gone; stored `custom` settings migrate to the matching preset

## Test quality rules

- Follow red-green-refactor. One failing test at a time.
- `markdown/` and `typst_engine/` are plain Rust (no `tauri` imports) — tests run with `cargo test`.
- Frontend pure modules (`lib/*.js`) testable with Vitest.
- Component bodies that can render statically are covered with `react-dom/server`; portal-based wrappers are left to the E2E layer.
- Slow engine tests marked with `#[ignore]`.
- No untested production code.