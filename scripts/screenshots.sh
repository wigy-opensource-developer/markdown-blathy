#!/usr/bin/env bash
# Regenerate docs/screenshots/ headlessly from the real app.
#
# Sets up a virtual display with a window manager (openbox, for the title bar),
# drives the app through tauri-driver + WebDriver to each state, and captures
# the decorated window with maim, compositing a soft shadow on white via
# ImageMagick. See CONTRIBUTING.md, "Refresh the screenshots".
#
# Usage: scripts/screenshots.sh [path-to-app-binary]
#   binary  default: target/release/MarkdownBláthy if built, else the debug
#           binary with embedded dist/ (build it first:
#           npm run build && cargo build -p markdown-blathy-tauri
#           --features tauri/custom-protocol).
#
# Requires: xvfb xauth dbus-x11 openbox maim xdotool imagemagick
#           webkit2gtk-driver tauri-driver node
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BINARY="${1:-}"
if [[ -z "$BINARY" ]]; then
  if [[ -x "target/release/MarkdownBláthy" ]]; then
    BINARY="target/release/MarkdownBláthy"
  elif [[ -x "target/debug/markdown-blathy-tauri" ]]; then
    BINARY="target/debug/markdown-blathy-tauri"
  else
    echo "error: no app binary found; build one first (see header)" >&2
    exit 1
  fi
fi
BINARY="$(realpath "$BINARY")"

for cmd in Xvfb openbox maim xdotool convert tauri-driver node; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "error: missing '$cmd' (see CONTRIBUTING.md)" >&2; exit 1; }
done

DISPLAY_NUM=99
export DISPLAY=":${DISPLAY_NUM}"
RUNTIME_DIR="$(mktemp -d)"; chmod 700 "$RUNTIME_DIR"
export XDG_RUNTIME_DIR="$RUNTIME_DIR"
# Isolate app data so persisted settings (e.g. theme) never leak between runs.
export XDG_DATA_HOME="$(mktemp -d)"
export WEBKIT_DISABLE_COMPOSITING_MODE=1

PIDS=()
cleanup() {
  for pid in "${PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
  rm -rf "$RUNTIME_DIR"
}
trap cleanup EXIT

Xvfb "$DISPLAY" -screen 0 1280x900x24 -nolisten tcp &
PIDS+=($!)
sleep 1
openbox &
PIDS+=($!)
sleep 0.5

tauri-driver --native-driver "$(command -v WebKitWebDriver)" --port 4444 &
PIDS+=($!)
for _ in $(seq 1 40); do
  curl -s -o /dev/null http://127.0.0.1:4444/status && break
  sleep 0.5
done

node scripts/screenshots.mjs "$BINARY"
echo "screenshots written to docs/screenshots/"
