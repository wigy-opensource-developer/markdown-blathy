// Contracts of the mounted-only page cache: SVGs exist only for the visible
// window, fetches deduplicate, a new render seq resets everything, and late
// or stale responses never resurrect unmounted pages.
import { describe, expect, it } from "vitest";
import { initialPages, nextViewport, applyFetch, failFetch } from "./previewPages.js";

const range = (first, last) => {
  const out = [];
  for (let i = first; i <= last; i++) out.push(i);
  return out;
};

describe("previewPages", () => {
  it("a new viewport fetches the visible pages once", () => {
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 2));
    expect(toFetch).toEqual([0, 1, 2]);
    expect(state.seq).toBe(1);
    expect([...state.inflight]).toEqual([0, 1, 2]);
    // The same viewport again (e.g. StrictMode's double effect): nothing new.
    const again = nextViewport(state, 1, range(0, 2));
    expect(again.toFetch).toEqual([]);
  });

  it("landed pages are held while visible", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [
      { index: 0, svg: "<svg>0</svg>" },
      { index: 1, svg: "<svg>1</svg>" },
      { index: 2, svg: "<svg>2</svg>" },
    ]);
    expect(state.svgs.get(1)).toBe("<svg>1</svg>");
    expect(state.inflight.size).toBe(0);
    // Re-reporting the viewport does not refetch held pages.
    expect(nextViewport(state, 1, range(0, 2)).toFetch).toEqual([]);
  });

  it("scrolling away drops the markup; scrolling back refetches", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [{ index: 0, svg: "<svg>0</svg>" }]);
    // Scroll far down: page 0 leaves the window → its SVG is dropped.
    const moved = nextViewport(state, 1, range(5, 7));
    expect(moved.state.svgs.size).toBe(0);
    expect(moved.toFetch).toEqual([5, 6, 7]);
    // Scroll back: page 0 is gone from RAM → fetched again (cheap export).
    const back = nextViewport(moved.state, 1, range(0, 2));
    expect(back.toFetch).toEqual([0, 1, 2]);
  });

  it("a page that scrolled out before its fetch landed is not stored", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = nextViewport(state, 1, range(5, 7)).state; // scroll while fetching
    state = applyFetch(state, 1, [0, 1, 2], [{ index: 0, svg: "<svg>0</svg>" }]);
    expect(state.svgs.size).toBe(0); // dropped — not visible any more
    expect([...state.inflight]).toEqual([5, 6, 7]); // old marks cleared
  });

  it("a new render seq resets the cache and the in-flight marks", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = applyFetch(state, 1, [0, 1, 2], [{ index: 0, svg: "<svg>0</svg>" }]);
    const fresh = nextViewport(state, 2, range(0, 2));
    expect(fresh.state.svgs.size).toBe(0); // old document's markup gone
    expect(fresh.toFetch).toEqual([0, 1, 2]);
    // A late response from the old seq changes nothing.
    const late = applyFetch(fresh.state, 1, [0, 1, 2], [{ index: 1, svg: "old" }]);
    expect(late).toBe(fresh.state);
  });

  it("a failed fetch clears its marks so the next viewport retries", () => {
    let { state } = nextViewport(initialPages(), 1, range(0, 2));
    state = failFetch(state, 1, [0, 1, 2]);
    expect(state.inflight.size).toBe(0);
    expect(nextViewport(state, 1, range(0, 2)).toFetch).toEqual([0, 1, 2]);
    // A failure for a superseded seq is ignored.
    expect(failFetch(state, 9, [0])).toBe(state);
  });

  it("an empty viewport (unmeasured pane) holds and fetches nothing", () => {
    const { state, toFetch } = nextViewport(initialPages(), 1, []);
    expect(toFetch).toEqual([]);
    expect(state.svgs.size).toBe(0);
  });

  it("a render's seeded window is held without refetching (atomic swap)", () => {
    const seed = [
      { index: 0, svg: "<svg>0</svg>" },
      { index: 1, svg: "<svg>1</svg>" },
    ];
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 2), seed);
    expect(state.svgs.get(0)).toBe("<svg>0</svg>");
    expect(state.svgs.get(1)).toBe("<svg>1</svg>");
    expect(toFetch).toEqual([2]); // only the unseeded page of the window
  });

  it("seeded pages outside the visible window are dropped", () => {
    const seed = [{ index: 9, svg: "<svg>9</svg>" }];
    const { state, toFetch } = nextViewport(initialPages(), 1, range(0, 1), seed);
    expect(state.svgs.size).toBe(0);
    expect(toFetch).toEqual([0, 1]);
  });

  it("a seed is ignored once the document is showing (viewport moves)", () => {
    const seed = [{ index: 0, svg: "<svg>0</svg>" }];
    let { state } = nextViewport(initialPages(), 1, range(0, 1), seed);
    // Scrolling within the same document must not resurrect seed data.
    const moved = nextViewport(state, 1, range(1, 2), seed);
    expect(moved.state.svgs.has(0)).toBe(false);
    expect(moved.toFetch).toEqual([2]);
  });
});
