// Geometry contracts of the virtualized preview: sheets scale to the pane
// width keeping aspect ratio, the full scroll height is known from metadata
// alone, and the visible window (± overscan) is what mounts.
import { describe, expect, it } from "vitest";
import {
  computeLayout,
  visibleRange,
  MAX_PAGE_WIDTH,
  PAGE_GAP,
  PAGE_PAD,
} from "./previewWindow.js";

// A4-ish points; two pages of different height to catch per-page scaling.
const SIZES = [
  [600, 900],
  [600, 1200],
  [600, 900],
];

describe("computeLayout", () => {
  it("scales sheets to the pane width, keeping aspect ratio", () => {
    const { slots } = computeLayout(SIZES, 400);
    expect(slots[0]).toEqual({ top: PAGE_PAD, width: 400, height: 600 });
    expect(slots[1].height).toBe(800); // 1200/600 * 400
    expect(slots[2].top).toBe(PAGE_PAD + 600 + PAGE_GAP + 800 + PAGE_GAP);
  });

  it("caps the sheet width like the old .page max-width", () => {
    const { slots } = computeLayout(SIZES, 2000);
    expect(slots[0].width).toBe(MAX_PAGE_WIDTH);
    expect(slots[0].height).toBe((900 / 600) * MAX_PAGE_WIDTH);
  });

  it("knows the total scroll height before any SVG exists", () => {
    const { slots, totalHeight } = computeLayout(SIZES, 400);
    const last = slots[slots.length - 1];
    expect(totalHeight).toBe(last.top + last.height + PAGE_PAD);
  });

  it("is empty for no pages or an unmeasured pane", () => {
    expect(computeLayout([], 400)).toEqual({ slots: [], totalHeight: 0 });
    expect(computeLayout(SIZES, 0)).toEqual({ slots: [], totalHeight: 0 });
    expect(computeLayout(undefined, 400)).toEqual({ slots: [], totalHeight: 0 });
  });

  it("tolerates a degenerate zero-width page", () => {
    const { slots } = computeLayout([[0, 900]], 400);
    expect(slots[0].height).toBe(0);
  });
});

describe("visibleRange", () => {
  const layout = computeLayout(SIZES, 400); // heights 600, 800, 600

  it("mounts the pages intersecting the viewport, plus overscan", () => {
    // Viewport [24, 424): page 0 only; overscan 2 reaches page 2.
    expect(visibleRange(layout, 0, 400)).toEqual({ first: 0, last: 2 });
  });

  it("follows the scroll window down the document", () => {
    // Page tops: 24, 648, 1472. Scroll to 700, viewport 400 → [700, 1100)
    // intersects page 1 only; overscan 1 → pages 0..2.
    expect(visibleRange(layout, 700, 400, 1)).toEqual({ first: 0, last: 2 });
    // Scroll to 1500 → page 2; overscan 0 → page 2 alone.
    expect(visibleRange(layout, 1500, 400, 0)).toEqual({ first: 2, last: 2 });
  });

  it("clamps overscan to the document bounds", () => {
    expect(visibleRange(layout, 0, 400, 99)).toEqual({ first: 0, last: 2 });
    expect(visibleRange(layout, 1500, 400, 99)).toEqual({ first: 0, last: 2 });
  });

  it("clamps a scroll past the end to the last page", () => {
    expect(visibleRange(layout, 99999, 400, 0)).toEqual({ first: 2, last: 2 });
  });

  it("is empty for an empty layout or an unmeasured viewport", () => {
    expect(visibleRange({ slots: [], totalHeight: 0 }, 0, 400)).toEqual({
      first: 0,
      last: -1,
    });
    expect(visibleRange(layout, 0, 0)).toEqual({ first: 0, last: -1 });
  });

  it("a viewport taller than the document mounts everything", () => {
    expect(visibleRange(layout, 0, 100000, 0)).toEqual({ first: 0, last: 2 });
  });
});
