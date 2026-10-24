# Contributing

## Prerequisites

- Node.js ≥ 22 (LTS; v24 recommended, and used by CI)
- Rust stable
- `cargo-deny` (`cargo install cargo-deny --locked`, or via your package manager)
- **Linux:** Tauri's native development headers —
  `libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
  libdbus-1-dev pkg-config build-essential libssl-dev`
- **Linux headless/WebDriver (optional):** `xvfb xauth dbus-x11
  webkit2gtk-driver` — see [TESTING.md](TESTING.md) for the headless smoke
  test and the plan for completing the E2E suite.
- **macOS:** Xcode Command Line Tools

On minimal Linux containers, install the packages above before running the Rust
checks. In particular, `libdbus-1-dev` is required by Tauri's Linux stack
(`libdbus-sys`); without it `cargo test`/`cargo clippy` fail while compiling a
dependency. On Debian/Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libdbus-1-dev pkg-config build-essential libssl-dev
```

For headless release smoke tests or WebDriver, add `xvfb xauth dbus-x11
webkit2gtk-driver`; the exact commands and the remaining E2E work are
documented in [TESTING.md](TESTING.md#headless-test-environment).

## Development

```bash
npm install
npm run tauri dev          # Vite hot-reload + the desktop app
```

## Quality checks

Run the full gate from the repo root:

```bash
./check.sh
```

## Production build

```bash
npm run tauri build
```

Output lands in `src-tauri/target/release/bundle/`.

## How to release a new version

Preparation happens **manually on the dev machine**; CI only packages the
release and opens a draft. The order matters: the metainfo references
screenshots by raw tag URL (`docs/screenshots/…` at `v<version>`, SPEC §5
M8 task 2), so they must be in the tagged commit.

### Tooling

- **git-cliff** — changelog from the conventional-commit history
  (`cargo install git-cliff --locked`, or your package manager). Configured by
  `cliff.toml` in the repo root (v-prefixed tag pattern, header marker).
- **Screenshot capture** — WebdriverIO with `@wdio/tauri-service` (default
  embedded provider via `tauri-plugin-wdio-webdriver`; `tauri-driver` +
  `webkit2gtk-driver` works on Linux too), plus `xvfb`, `openbox`/`mutter`,
  `picom`, `xdotool`, `maim` (or ImageMagick `import`) and ImageMagick. See
  <https://v2.tauri.app/develop/tests/webdriver/>.

### 1. Pre-flight

`./check.sh` passes, the working tree is clean, and pick the version:

```bash
VERSION=v0.2.0   # tags are v-prefixed
```

### 2. Refresh the screenshots

Screenshots always come from the real binary — a mocked `render_preview`
cannot reproduce Typst layout, and someone has to pick a good window state
and content.

1. Start the release binary on a virtual display with a window manager and
   compositor (`picom`) so the window gets its native shadow. The app takes
   no CLI file argument, so the document is the built-in sample or typed via
   the WebDriver session.
2. Drive it to the state to show (open the Document or Page popover, …) and
   wait for the render.
3. Capture the app window by id with `maim` (ImageMagick `import` works
   too) — `browser.takeScreenshot()` only captures the webview viewport.
4. Follow Flathub's screenshot quality guidelines: window-only, no desktop
   background, native shadow and rounded corners, ~1000×700 (2000×1400
   HiDPI), one-sentence captions, first screenshot `type="default"`.
5. Save to `docs/screenshots/` and update the metainfo `<screenshots>`.

Validate the metainfo locally with `AS_VALIDATE_NONET=1` set —
`appstreamcli` otherwise fetches the screenshot URLs, which do not resolve
until the tag is pushed.

### 3. Generate the changelog

```bash
git-cliff --unreleased --tag "$VERSION" --prepend CHANGELOG.md
```

`--prepend` is **append-only**: it prepends the new version's section and
leaves every existing line untouched, so past release sections you have
hand-edited are never overwritten. (The contrast is `--output`, which
rewrites the whole file — do not use it for routine releases.) The
`cliff.toml` `header_marker` lets git-cliff replace a templated header on
later runs without disturbing the body.

The repo has no tags yet, so the first run covers the whole history. For that
first run only, `CHANGELOG.md` does not exist yet and `--prepend` cannot
create it — use `--output CHANGELOG.md` once, then switch to `--prepend` for
every later release. Commits follow conventional commits (`feat:`,
`fix(scope):`, `refactor:`, `chore:`, `doc:`); any commit git-cliff reports as
skipped (e.g. a subject with a parenthetical it misreads as a body) must be
added to the section by hand. Add the matching
`<release version="…" date="…">` entry to the metainfo as well.

### 4. Bump the version

Keep these in sync:

- `package.json` → `version`
- `src-tauri/Cargo.toml` → `[package] version`
- `src-tauri/tauri.conf.json` → `version`
- the metainfo `<releases>` entry

### 5. Commit, tag, push

```bash
git add -A
git commit -m "chore(release): $VERSION"
git tag -a "$VERSION" -m "$VERSION"
git push --follow-tags
```

### 6. Review the draft release

The tag-triggered workflow (SPEC §5, M8 task 3) builds `.deb`, `.rpm`,
the AppImage, the macOS `.dmg` and the Windows installers — each in its own
reusable workflow — and opens an **unpublished** GitHub release with the
artifacts and git-cliff release notes for the tag. Review the notes and
assets, then publish manually — nothing is public until you do.

The macOS and Windows bundles are unsigned by default (Gatekeeper/SmartScreen
warn on first launch). Adding the `APPLE_*` secrets (macOS) or a code-signing
certificate (Windows) makes Tauri sign them without further workflow changes.

Any single artifact can be rebuilt without cutting a release: run its workflow
from the Actions tab (`Linux .deb`, `Linux .rpm`, `Linux AppImage`, `macOS`,
`Windows`).

## Specification

Three documents describe the project:

- **SPEC.md** — Product overview, architecture, flavor support, milestones, limitations. Read this first.
- **IMPLEMENTATION.md** — Module structure, API contracts, converter mapping, engine details, implementation decisions. The implementor's guide.
- **TESTING.md** — Test matrix with priorities, milestone acceptance checklists, test quality rules.
