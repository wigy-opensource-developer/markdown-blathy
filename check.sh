#!/usr/bin/env bash
# Quality gate: frontend build + Rust workspace checks.
# See SPEC.md, IMPLEMENTATION.md, and TESTING.md for project structure.
# The Cargo workspace root is the repo root; the Tauri app is a member under
# src-tauri/, and the headless core/CLI live under crates/.
# Usage: ./check.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

for cmd in npm cargo cargo-deny; do
  command -v "$cmd" >/dev/null 2>&1 || {
    echo "error: '$cmd' not found on PATH" >&2
    if [ "$cmd" = "cargo-deny" ]; then
      echo "       install it with: cargo install cargo-deny --locked" >&2
    fi
    exit 1
  }
done

echo "==> npm run build (frontend)"
npm run build

echo "==> npm test (frontend unit tests)"
npm test

echo "==> npm run check:bundle (dist guards: no WOFF 1.0, no unused font weights, no js/css grammars)"
npm run check:bundle

echo "==> npm run check:licenses (frontend dependencies)"
npm run check:licenses

echo "==> cargo fmt --check (workspace)"
cargo fmt --all --check

echo "==> cargo clippy (workspace: all targets, warnings as errors)"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test (workspace)"
cargo test --workspace

echo "==> cargo deny check (workspace: advisories, bans, licenses, sources)"
cargo deny --config src-tauri/deny.toml check --hide-inclusion-graph

echo "All checks passed."
