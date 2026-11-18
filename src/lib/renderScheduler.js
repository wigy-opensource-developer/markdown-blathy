// Single-flight render scheduler: the ONE owner of "when does the preview
// render start". It merges three concerns that used to be spread across
// App.jsx (the 200 ms debounce) and lib/previewFreeze.js (the modal freeze):
//
//   1. Debounce — an edit (keystroke, option change) arms a 200 ms timer;
//      only a pause renders.
//   2. Single flight — while a render is in flight nothing new is started
//      *or scheduled*: edits only mark the cycle `owed` and refresh the
//      last-edit timestamp. The Typst compile runs off the UI thread, so
//      typing continues regardless; the scheduler just refuses to pile up
//      compiles whose older responses would be thrown away anyway.
//   3. Catch-up — when the render settles (result, failure, or invalidated
//      response), a new render starts iff there were edits since it began
//      AND the last edit is ≥ debounceMs old; if the user typed more
//      recently, the remaining debounce is armed instead. Many edits
//      collapse into one catch-up.
//
// The modal freeze lives in the same machine: while `paused` nothing
// starts and a finished result is stashed; closing catches up once (a
// newer change supersedes the stash). Loads use `requestNow()`: no
// debounce, and an in-flight response is invalidated through the seq
// guard — but the load's own render still waits for the in-flight one to
// settle, keeping the cycle single-flight even across loads.
//
// Save and close never consult the scheduler: they read the live buffer
// and the dirty ref, so an ongoing render cannot delay them.
//
// Pure state machine: `render` (async; resolves with the result, or null
// when the response was dropped/failed), `apply` (the heavy commit) and
// `invalidate` (bump the seq) are injected; timers are the globals, which
// vitest fakes.
export function createRenderScheduler({
  debounceMs = 200,
  render,
  apply,
  invalidate = () => {},
}) {
  let paused = false; // a modal is open: nothing starts, results stash
  let inFlight = false; // a render is running
  let owed = false; // edits that no started render covers yet
  let asap = false; // the owed render skips the debounce (a load)
  let lastEdit = 0; // when the last request came in
  let timer = null; // the pending debounce (only armed while idle+unpaused)
  let stashed = null; // a result that landed while paused

  function clearTimer() {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  }

  /** Start the owed render now, or arm the remaining debounce. While a
   *  render is in flight or a modal is open there is nothing to schedule:
   *  `finish`/`close` re-runs this check when the blocker goes away. */
  function schedule() {
    clearTimer();
    if (!owed || inFlight || paused) return;
    const delay = asap ? 0 : Math.max(0, lastEdit + debounceMs - Date.now());
    if (delay === 0) {
      start();
      return;
    }
    timer = setTimeout(() => {
      timer = null;
      // Defensive: open()/start() clear the timer, so a live one always
      // fires idle and unpaused. If that ever changes, the debt survives.
      if (paused || inFlight) return;
      start();
    }, delay);
  }

  function start() {
    clearTimer();
    owed = false;
    asap = false;
    inFlight = true;
    let result;
    try {
      result = render();
    } catch {
      result = null; // a synchronous throw is a failed render
    }
    Promise.resolve(result).then(finish, () => finish(null));
  }

  /** A render settled: show the result (or stash it while paused), then
   *  run the catch-up check — edits since the start + a ≥ debounceMs old
   *  last edit start the next render at once, newer edits arm the
   *  remaining debounce, no edits leave the cycle idle. */
  function finish(result) {
    inFlight = false;
    if (result != null) {
      if (paused) stashed = result;
      else apply(result);
    }
    schedule();
  }

  return {
    /** An edit needs a render: debounce it, or just remember it while a
     *  render is in flight / a modal is open. */
    request() {
      lastEdit = Date.now();
      owed = true;
      asap = false;
      schedule();
    },
    /** A freshly loaded document: render without the debounce. An
     *  in-flight response is invalidated (it shows the old document); the
     *  load's render starts as soon as that one settles. A keystroke after
     *  the load folds into the same owed render, debounced normally. */
    requestNow() {
      lastEdit = Date.now();
      owed = true;
      asap = true;
      invalidate();
      schedule();
    },
    /** A modal opened: freeze; the pending debounce is dropped, the debt
     *  stays. An in-flight render may still finish — its result stashes. */
    open() {
      paused = true;
      clearTimer();
    },
    /** A modal closed: catch up once — the owed render (its deadline is
     *  measured from the last edit, so a long-open modal renders at once),
     *  or the stashed result. An unpaired close (a widget notifying its
     *  initial closed state) is inert. */
    close() {
      if (!paused) return;
      paused = false;
      if (owed) {
        stashed = null; // a newer change supersedes the stash
        schedule();
      } else if (stashed) {
        apply(stashed);
        stashed = null;
      }
    },
    /** Unmount: drop the pending debounce. An in-flight render's late
     *  finish is harmless (its commit lands on a torn-down tree). */
    dispose() {
      clearTimer();
    },
  };
}
