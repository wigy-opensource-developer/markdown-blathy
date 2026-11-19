// Geometry for the virtualized preview pane.
//
// A render lands as METADATA only: the page count and each page's intrinsic
// (width, height) in points. From that the preview lays out every page as a
// placeholder sheet with the correct aspect ratio — the full scroll height
// is known before any SVG is exported, so the scrollbar never jumps when
// pages arrive. Only the visible window (± overscan) is mounted; the rest
// stay placeholders until scrolled to (lib/previewPages.js owns the SVGs).
//
// Pure functions, unit-tested; the constants mirror styles.css (.page).

/** Breathing room above the first sheet and below the last (px). */
export const PAGE_PAD = 24;
/** Space between sheets (px) — the old `.page` margin-bottom. */
export const PAGE_GAP = 24;
/** Sheets never grow past this width (px) — the old `.page` max-width. */
export const MAX_PAGE_WIDTH = 820;

/**
 * Lay out `pageSizes` ([width, height] points per page) for a pane whose
 * content box is `contentWidth` px wide. Every sheet scales to the pane
 * width (capped at MAX_PAGE_WIDTH), keeping its aspect ratio — exactly what
 * `width:100%; height:auto` did when the SVGs set the size themselves.
 * Returns `{ slots: [{ top, width, height }], totalHeight }`; an empty or
 * unmeasured pane yields an empty layout.
 */
export function computeLayout(pageSizes, contentWidth) {
  if (!Array.isArray(pageSizes) || pageSizes.length === 0) {
    return { slots: [], totalHeight: 0 };
  }
  if (!(contentWidth > 0)) {
    return { slots: [], totalHeight: 0 };
  }
  const width = Math.min(contentWidth, MAX_PAGE_WIDTH);
  const slots = [];
  let top = PAGE_PAD;
  for (const [w, h] of pageSizes) {
    const height = w > 0 ? (h / w) * width : 0;
    slots.push({ top, width, height });
    top += height + PAGE_GAP;
  }
  // The last gap becomes the bottom padding.
  return { slots, totalHeight: top - PAGE_GAP + PAGE_PAD };
}

/**
 * The inclusive page range intersecting the scroll window
 * `[scrollTop, scrollTop + viewportHeight)`, expanded by `overscan` pages
 * on both sides and clamped to the document. An empty layout or an
 * unmeasured (zero-height) viewport yields an empty range (`last < first`)
 * so nothing mounts or fetches before the first measurement.
 */
export function visibleRange(layout, scrollTop, viewportHeight, overscan = 2) {
  const n = layout.slots.length;
  if (n === 0 || !(viewportHeight > 0)) {
    return { first: 0, last: -1 };
  }
  const top = Math.max(0, scrollTop);
  const bottom = top + viewportHeight;
  let first = 0;
  while (first < n && layout.slots[first].top + layout.slots[first].height < top) {
    first++;
  }
  first = Math.min(first, n - 1); // scrolled past the end (clamp)
  let last = first;
  while (last + 1 < n && layout.slots[last + 1].top < bottom) {
    last++;
  }
  return {
    first: Math.max(0, first - overscan),
    last: Math.min(n - 1, last + overscan),
  };
}
