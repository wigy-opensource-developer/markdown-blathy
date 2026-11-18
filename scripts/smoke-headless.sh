#!/usr/bin/env bash
# Headless GUI smoke test (TESTING.md, "Headless test environment").
#
# Starts the real GUI binary on a virtual X display and checks that it stays
# alive: the window — and with it the WebKitGTK webview — must initialize, so
# an immediate exit or crash fails the test. Exit status 124 from `timeout`
# is the success case: the app was still running when the deadline hit.
#
# Usage: scripts/smoke-headless.sh [binary] [timeout-seconds]
#   binary        default target/release/MarkdownBláthy
#   timeout       default 15
#
# Requires the headless stack from TESTING.md (xvfb, xauth, dbus-x11); the
# CI `headless-smoke` job runs this script on every push and PR.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="${1:-$ROOT/target/release/MarkdownBláthy}"
TIMEOUT="${2:-15}"

for cmd in xvfb-run dbus-run-session timeout; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "error: '$cmd' not found — install the headless stack from TESTING.md:" >&2
    echo "       sudo apt-get install -y --no-install-recommends xvfb xauth dbus-x11 webkit2gtk-driver" >&2
    exit 1
  fi
done

if [[ ! -x "$BINARY" ]]; then
  echo "error: GUI binary not found or not executable: $BINARY" >&2
  echo "       build it first, e.g. npm run tauri build -- --no-bundle" >&2
  exit 1
fi

RUNTIME_DIR="$(mktemp -d)"
LOG="$(mktemp)"
trap 'rm -rf "$RUNTIME_DIR" "$LOG"' EXIT
chmod 700 "$RUNTIME_DIR"

set +e
env XDG_RUNTIME_DIR="$RUNTIME_DIR" \
  xvfb-run -a dbus-run-session -- \
  env WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  timeout "$TIMEOUT" "$BINARY" >"$LOG" 2>&1
status=$?
set -e

if [[ $status -eq 124 ]]; then
  echo "ok: GUI stayed alive for ${TIMEOUT}s under Xvfb ($BINARY)"
  exit 0
fi

echo "error: GUI exited before the ${TIMEOUT}s deadline (status $status) — the window did not initialize" >&2
echo "--- output:" >&2
tail -50 "$LOG" >&2
exit 1
