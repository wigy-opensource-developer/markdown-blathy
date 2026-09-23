// Delayed-visibility feedback for live preview renders.
//
// Most keystroke renders land well under half a second; a spinner for those
// would only flicker. A big document, though, pends for a noticeable time —
// and with the last-good pages still on screen the user cannot tell "settled"
// from "still cooking". So the "Rendering…" note appears only when a render
// has been pending for `delayMs` in a row.
//
// Pure state machine (no React, timers injected via `setTimeout`, which the
// app supplies natively and vitest fakes):
//
//   start()  — a render began (call from the same place `started()` marks);
//              repeated calls while pending do NOT restart the deadline.
//   stop()   — the render landed, was dropped as stale, or failed.
//   notify — called with `true` when the note should appear, `false` when
//              it should go away; never fired for a short episode.
//
// A dropped stale response (App.jsx's seq check) must NOT call `stop()`:
// the render that superseded it is still the live episode — its `start()`
// merged into the one already armed, and only its own completion stops.
export function createRenderStatus({ delayMs, notify }) {
  let timer = null; // set while pending but not yet visible
  let visible = false;

  return {
    start() {
      if (visible || timer) return; // deadline armed once per episode
      timer = setTimeout(() => {
        timer = null;
        visible = true;
        notify(true);
      }, delayMs);
    },
    stop() {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
      if (visible) {
        visible = false;
        notify(false);
      }
    },
  };
}
