# tauri-md-editor

A desktop Markdown editor with live preview and on-demand PDF export.
The preview and the PDF are produced by the same Typst compilation pipeline — true WYSIWYG.

## Tech stack

- **Frontend:** React 19 · Mantine 8 · Tabler Icons · Vite
- **Backend:** Rust · Tauri 2 · comrak · Typst

## Quick start

```bash
npm install
npm run tauri dev
```

## Build

```bash
npm run tauri build
```

The full specification lives in [`SPEC.md`](SPEC.md).

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
