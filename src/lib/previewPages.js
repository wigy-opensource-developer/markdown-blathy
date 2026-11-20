// Keep-all page cache for the virtualized preview.
//
// The single source of truth is the backend's ONE cached compiled document
// (LastRender); page rasters (PNG, base64) + link rects are exported from it
// on demand. Every page exported for the current render seq is KEPT (not just
// the visible window): scrolling back needs no IPC round-trip, and because
// the page also stays mounted, no re-decode either. Memory is bounded by one
// render — a new seq resets everything, so nothing accumulates across edits.
//
// Pure reducer helpers, unit-tested. State:
//   { seq, visible: [indices], pages: Map<index, {png, links}>, inflight: Set }
// `inflight` deduplicates fetches across viewport events and StrictMode's
// double effects; a stale seq is ignored entirely. Fetch priority is
// top-first so the pages under the viewport are requested before the rest.

export function initialPages() {
  return { seq: null, visible: [], pages: new Map(), inflight: new Set() };
}

/** Order page indices top-first (ascending): the pages at the top of the
 * viewport are requested before lower ones, giving fast feedback where the
 * user is looking. */
export function fetchPriority(indices) {
  return [...indices].sort((a, b) => a - b);
}

/**
 * The viewport moved (or a new render landed): keep every page already held
 * for this seq, reset everything on a seq change (seeding any pages the
 * render response already exported), and report which visible pages need
 * fetching (not held, not already in flight — those are marked now), in
 * top-first priority. Returns `{ state, toFetch }`.
 */
export function nextViewport(state, seq, visible, seed = []) {
  const sameDoc = state.seq === seq;
  const pages = new Map(sameDoc ? state.pages : []);
  const inflight = new Set(sameDoc ? state.inflight : []);
  if (!sameDoc) {
    // A new render: the response may carry the initial window's rasters
    // (exported with the compile) — seed them so the swap is atomic and
    // nothing is refetched.
    for (const { index, png, links } of seed) {
      pages.set(index, { png, links: links ?? [] });
    }
  }
  const toFetch = [];
  for (const i of fetchPriority(visible)) {
    if (!pages.has(i) && !inflight.has(i)) {
      toFetch.push(i);
      inflight.add(i);
    }
  }
  return { state: { seq, visible: [...visible], pages, inflight }, toFetch };
}

/**
 * A fetch landed: clear its in-flight marks and keep the pages for the whole
 * seq (they stay cached for instant scroll-back). A response for a
 * superseded seq is ignored unchanged.
 */
export function applyFetch(state, seq, requested, pages) {
  if (state.seq !== seq) return state;
  const inflight = new Set(state.inflight);
  for (const i of requested) inflight.delete(i);
  const next = new Map(state.pages);
  for (const { index, png, links } of pages) {
    next.set(index, { png, links: links ?? [] });
  }
  return { ...state, pages: next, inflight };
}

/** A fetch failed: clear its in-flight marks so the next viewport event
 *  retries. A superseded seq is ignored unchanged. */
export function failFetch(state, seq, requested) {
  if (state.seq !== seq) return state;
  const inflight = new Set(state.inflight);
  for (const i of requested) inflight.delete(i);
  return { ...state, inflight };
}
