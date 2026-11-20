// Contracts of the keep-all page cache: every page exported for the current
// render seq is held (instant scroll-back, no re-export IPC), fetches
// deduplicate and go top-first, a new render seq resets everything, and
// stale responses never resurrect a superseded document.
import { describe, expect, it } from "vitest";
import { initialPages, nextViewport, applyFetch, failFetch, fetchPriority } from "./previewPages.js";

const range = (first, last) => {
  const out = [];
  for (let i = first; i <= last; i++) out.push(i);
  return out;
};
const page = (index, png = `png${index}`, links = []) => ({ index, png, links });

describe("previewPages", () => {
  it("a new viewport fetches the visible pages once, top-first", () => {
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 2));
    expect(toFetch).toEqual([0, 1, 2]);
    expect(state.seq).toBe(1);
    expect([...state.inflight]).toEqual([0, 1, 2]);
    // The same viewport again (e.g. StrictMode's double effect): nothing new.
    const again = nextViewport(state, 1, range(0, 2));
    expect(again.toFetch).toEqual([]);
  });

  it("fetchPriority orders indices top-first", () => {
    expect(fetchPriority([5, 2, 4, 3])).toEqual([2, 3, 4, 5]);
  });

  it("landed pages are held with their links", () => {
    const links = [{ x: 1, y: 2, w: 3, h: 4, href: "https://example.com" }];
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [page(0), page(1, "png1", links), page(2)]);
    expect(state.pages.get(1).png).toBe("png1");
    expect(state.pages.get(1).links).toEqual(links);
    expect(state.pages.get(0).links).toEqual([]);
    expect(state.inflight.size).toBe(0);
    expect(nextViewport(state, 1, range(0, 2)).toFetch).toEqual([]);
  });

  it("scrolling away keeps pages; scrolling back does not refetch", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [page(0), page(1), page(2)]);
    // Scroll far down: page 0 leaves the window but stays cached.
    const moved = nextViewport(state, 1, range(5, 7));
    expect(moved.state.pages.has(0)).toBe(true);
    expect(moved.toFetch).toEqual([5, 6, 7]);
    // Scroll back: page 0 is still held → no refetch, instant remount.
    const back = nextViewport(moved.state, 1, range(0, 2));
    expect(back.toFetch).toEqual([]);
    expect(back.state.pages.get(0).png).toBe("png0");
  });

  it("a fetch that lands after scrolling away is still cached", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = nextViewport(state, 1, range(5, 7)).state; // scroll while fetching
    state = applyFetch(state, 1, [0, 1, 2], [page(0)]);
    expect(state.pages.has(0)).toBe(true); // kept for scroll-back
    expect([...state.inflight]).toEqual([5, 6, 7]); // old marks cleared
  });

  it("a new render seq resets the cache and the in-flight marks", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [page(0)]);
    const fresh = nextViewport(state, 2, range(0, 2));
    expect(fresh.state.pages.size).toBe(0); // old document's pages gone
    expect(fresh.toFetch).toEqual([0, 1, 2]);
    // A late response from the old seq changes nothing.
    const late = applyFetch(fresh.state, 1, [0, 1, 2], [page(1, "old")]);
    expect(late).toBe(fresh.state);
  });

  it("a failed fetch clears its marks so the next viewport retries", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = failFetch(state, 1, [0, 1, 2]);
    expect(state.inflight.size).toBe(0);
    expect(nextViewport(state, 1, range(0, 2)).toFetch).toEqual([0, 1, 2]);
    expect(failFetch(state, 9, [0])).toBe(state); // superseded seq ignored
  });

  it("an empty viewport (unmeasured pane) holds and fetches nothing", () => {
    const { state, toFetch } = nextViewport(initialPages(), 1, []);
    expect(toFetch).toEqual([]);
    expect(state.pages.size).toBe(0);
  });

  it("a render's seeded window is held without refetching (atomic swap)", () => {
    const seed = [page(0), page(1)];
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 2), seed);
    expect(state.pages.get(0).png).toBe("png0");
    expect(state.pages.get(1).png).toBe("png1");
    expect(toFetch).toEqual([2]); // only the unseeded page of the window
  });

  it("seeded pages outside the window are kept but not refetched", () => {
    const seed = [page(9)];
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 1), seed);
    expect(state.pages.has(9)).toBe(true); // keep-all
    expect(toFetch).toEqual([0, 1]);
  });

  it("a held page survives viewport moves within the same document", () => {
    const seed = [page(0)];
    let { state } = nextViewport(initialPages(), 1, range(0, 1), seed);
    const moved = nextViewport(state, 1, range(1, 2), seed);
    expect(moved.state.pages.has(0)).toBe(true); // not dropped on scroll
    expect(moved.toFetch).toEqual([2]);
  });
});
