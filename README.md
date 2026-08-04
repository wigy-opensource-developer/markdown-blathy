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

## Known limitations

- **Legacy-codepage filenames** — on Linux systems using a non-UTF-8 locale (e.g. latin-2), a file's *name* is echoed over IPC as a lossy UTF-8 string. Contents load and saves still write to the correct file, but Save-after-Open may create a new file with a mangled name instead of overwriting. UTF-8-locale Linux, macOS and Windows are unaffected. Details in SPEC.md §7.4 / §13.

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
