# MarkdownBláthy

**Markdown, transformed.**

MarkdownBláthy is a desktop Markdown editor with live preview and on-demand PDF export. The preview and the PDF are produced by the same Typst compilation pipeline — true WYSIWYG.
The name commemorates Ottó Titusz Bláthy who among his other inventions became the co-inventor of the efficient alternating current transformer.

## Features

- **Split-screen editing** — Markdown on the left, live preview on the right.
- **Pixel-identical preview** — the preview is rendered by the same Typst compiler that produces the PDF, so what you see is what you get.
- **Export to PDF** — one click, system save dialog, opens in your default PDF viewer.
- **Multiple Markdown flavors** — CommonMark, GFM (GitHub Flavored Markdown), or Custom (footnotes, math, task lists).
- **LaTeX-style math** — inline and display math use familiar TeX/LaTeX notation (fractions, roots, matrices, common macros); Typst renders the preview and PDF.
- **Custom templates** — supply your own `.typ` template to restyle everything (colors, fonts, layout) while keeping the editing experience.
- **Heading IDs** — every heading gets an automatic anchor link; override with `{#custom-id}` syntax.
- **Footnotes** — inline references with note blocks at the bottom; clickable markers in the preview.
- **Settings persist** — your flavor, paper size, margins, and template are saved between sessions.

## Quick start

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

Output lands in `src-tauri/target/release/bundle/`.

## Tech stack

- **Frontend:** React 19 · Mantine 8 · Tabler Icons · Vite
- **Backend:** Rust · Tauri 2 · comrak · Typst

## Specification

The full implementation specification lives in [`SPEC.md`](SPEC.md).

## Known limitations

- **Legacy-codepage filenames** — on Linux systems using a non-UTF-8 locale (e.g. latin-2), a file's *name* is echoed over IPC as a lossy UTF-8 string. Contents load and saves still write to the correct file, but Save-after-Open may create a new file with a mangled name instead of overwriting. UTF-8-locale Linux, macOS and Windows are unaffected. Details in [SPEC.md](SPEC.md).

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
