// Mounted-only page cache for the virtualized preview.
//
// The single source of truth is the backend's ONE cached compiled document
// (LastRender); SVG strings are exported from it on demand and live here
// only for pages currently mounted (visible ± overscan). Scrolling away
// drops a page's markup; scrolling back re-exports it (cheap — no
// recompile). Nothing accumulates across renders: a new seq resets
// everything. This is deliberately not an SVG cache of the document —
// RAM holds the window, not the pages.
//
// Pure reducer helpers, unit-tested. State:
//   { seq, visible: [indices], svgs: Map<index, svg>, inflight: Set<index> }
// `inflight` deduplicates fetches across viewport events and StrictMode's
// double effects; a fetch that lands after its page scrolled out of the
// window is dropped, and a stale seq is ignored entirely.

export function initialPages() {
  return { seq: null, visible: [], svgs: new Map(), inflight: new Set() };
}

/**
 * The viewport moved (or a new render landed): prune SVGs to the visible
 * window, reset everything on a seq change (seeding any pages the render
 * response already exported), and report which visible pages need fetching
 * (not held, not already in flight — those are marked now).
 * Returns `{ state, toFetch }`.
 */
export function nextViewport(state, seq, visible, seed = []) {
  const sameDoc = state.seq === seq;
  const svgs = new Map();
  const inflight = new Set(sameDoc ? state.inflight : []);
  if (sameDoc) {
    for (const i of visible) {
      if (state.svgs.has(i)) svgs.set(i, state.svgs.get(i));
    }
  } else {
    // A new render: the response may carry the initial window's SVGs
    // (exported with the compile) — seed them so the swap is atomic and
    // nothing is refetched. Pages outside the window are dropped.
    for (const { index, svg } of seed) {
      if (visible.includes(index)) svgs.set(index, svg);
    }
  }
  const toFetch = [];
  for (const i of visible) {
    if (!svgs.has(i) && !inflight.has(i)) {
      toFetch.push(i);
      inflight.add(i);
    }
  }
  return { state: { seq, visible: [...visible], svgs, inflight }, toFetch };
}

/**
 * A fetch landed: clear its in-flight marks and keep only the pages still
 * visible. A response for a superseded seq is ignored unchanged.
 */
export function applyFetch(state, seq, requested, pages) {
  if (state.seq !== seq) return state;
  const inflight = new Set(state.inflight);
  for (const i of requested) inflight.delete(i);
  const svgs = new Map(state.svgs);
  for (const { index, svg } of pages) {
    if (state.visible.includes(index)) svgs.set(index, svg);
  }
  return { ...state, svgs, inflight };
}

/** A fetch failed: clear its in-flight marks so the next viewport event
 *  retries. A superseded seq is ignored unchanged. */
export function failFetch(state, seq, requested) {
  if (state.seq !== seq) return state;
  const inflight = new Set(state.inflight);
  for (const i of requested) inflight.delete(i);
  return { ...state, inflight };
}
