# Agents

Context for AI agents working on this repository.

## Overview

Desktop Markdown editor with Typst-powered live preview and PDF export.
See **README.md** for a quick overview and **CONTRIBUTING.md** for build/check commands.

## Key paths

- `SPEC.md` — full implementation spec
- `src-tauri/` — Rust backend (Tauri commands, comrak → Typst pipeline)
- `src/` — React frontend (Mantine UI, CodeMirror editor, Typst SVG preview)

## Before committing

Run the full quality gate from the repo root (it handles the `src-tauri/`
subdirectory for the cargo commands):

```bash
./check.sh
```
