// Preview freeze: wraps the debounced Typst renderer so the Document
// settings popover can be modal (§6.2). A full render blocks the UI thread
// and scales with the document, so while the popover is open nothing may
// render under the user's hands; changes made meanwhile collapse into ONE
// catch-up render on close. The App wires it to three call sites:
//
//   refreshPreview  → started() before the IPC, finish(result) after
//   requestPreview  → request()
//   settings opened → open(); settings closed → close()
//
// Three bits of state: `paused` (the popover is open), `owed` (something
// changed and a render must still run) and `stashed` (a result from a
// render that finished — uncancellable — while paused). The subtleties:
// `owed` is cleared by `started()`, when a scheduled render actually
// begins, not by `close()` — so a debounce dropped by `open()` keeps its
// debt, while one already in flight hands its result to `finish()` instead.
// And only a real pause can owe: an unpaired `close()` (the popover widget
// notifying its initial closed state) never schedules a double render.
export function createPreviewFreeze({ refresh, cancel, apply }) {
  let paused = false;
  let owed = false;
  let stashed = null;
  return {
    /** A document change needs a render; schedule it unless frozen. */
    request() {
      owed = true;
      if (!paused) refresh();
    },
    /** A scheduled render began; nothing is owed any more. */
    started() {
      owed = false;
    },
    /** A finished render: commit it, or stash it while frozen. */
    finish(result) {
      if (paused) stashed = result;
      else apply(result);
    },
    /** Popover opened: freeze and drop the pending debounce (still owed). */
    open() {
      paused = true;
      cancel();
    },
    /** Popover closed: show the owed render, or the stashed one — once. */
    close() {
      const catchUp = paused && owed;
      paused = false;
      if (catchUp) {
        // A newer change (e.g. Apply) supersedes any stashed render.
        stashed = null;
        refresh();
      } else if (stashed) {
        apply(stashed);
        stashed = null;
      }
    },
  };
}
