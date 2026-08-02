# Contributing

## Prerequisites

- Node.js ≥ 18 (LTS; v20 via nvm is fine)
- Rust stable
- `cargo-deny` (`cargo install cargo-deny --locked`, or via your package manager)
- **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev pkg-config build-essential libssl-dev`
- **macOS:** Xcode Command Line Tools

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
