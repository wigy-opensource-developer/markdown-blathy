#!/usr/bin/env bash
# Quality gate: frontend build + Rust checks.
# See SPEC.md, IMPLEMENTATION.md, and TESTING.md for project structure.
# The Rust crate lives in src-tauri/, so cargo runs must happen there.
# Usage: ./check.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$ROOT/src-tauri"

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

echo "==> npm run check:bundle (no WOFF 1.0 in dist — woff2Only plugin)"
npm run check:bundle

echo "==> npm run check:licenses (frontend dependencies)"
npm run check:licenses

echo "==> cargo fmt --check (src-tauri)"
(cd "$RUST_DIR" && cargo fmt --check)

echo "==> cargo clippy (src-tauri: all targets, warnings as errors)"
(cd "$RUST_DIR" && cargo clippy --all-targets -- -D warnings)

echo "==> cargo test (src-tauri)"
(cd "$RUST_DIR" && cargo test)

echo "==> cargo deny check (src-tauri: advisories, bans, licenses, sources)"
(cd "$RUST_DIR" && cargo deny check --hide-inclusion-graph)

echo "All checks passed."
