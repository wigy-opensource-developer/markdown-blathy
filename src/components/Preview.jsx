// Preview pane: a VIRTUALIZED list of white "sheets", one per printed page
// (§8.3 of SPEC.md).
//
// A render lands as metadata (page count + intrinsic sizes + paper color)
// PLUS the initial window's SVGs, exported from the same compile so the
// swap from the last-good pages is atomic — the old pages stay up, dimmed
// under the delayed pane note, until the new ones land. The backend keeps
// the ONE compiled document; scrolling exports further pages on demand
// (export_pages). This component lays out every page as a placeholder with
// the correct aspect ratio — the scroll height is stable from the moment a
// render lands — and mounts real SVG only for the visible window ± overscan
// (lib/previewWindow.js). Mounted-only markup lives in lib/previewPages.js:
// scrolling away drops a page's SVG, scrolling back re-exports it (cheap,
// no recompile). No commit ever carries the whole multi-MB preview again.
// A small current/total page badge floats in the pane's bottom-right
// (currentPageAt — the topmost visible page, no source mapping needed).
//
// The SVG strings come from our own Typst compile — never from user input
// directly — so injecting them as DOM is safe (same trust boundary as the
// preview renderer in general).
//
// Memoized: the metadata object only changes when a compile lands, so this
// component is skipped on unrelated state commits. The delayed `busy` flag
// (lib/renderStatus.js) only flips for renders long enough to deserve
// feedback: the last-good pages stay up, dimmed, under a tilted
// "Rendering…" note. `busy` also rides along as the parent section's
// `data-rendering` attribute (App.jsx), which drives the dimming CSS.
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { computeLayout, visibleRange, currentPageAt } from "../lib/previewWindow.js";
import { initialPages, nextViewport, applyFetch, failFetch } from "../lib/previewPages.js";

/** One sheet of the page list. With its SVG it is the printed page; without
 *  it, it is a placeholder in the template's paper color (the --sheet-bg
 *  variable, set from the render metadata) carrying the same tilted
 *  "Rendering…" note the pane shows for slow renders — aria-hidden, since
 *  the pane-level role="status" note already announces rendering. */
export function Sheet({ index, slot, svg }) {
  const style = {
    top: slot.top,
    width: slot.width,
    height: slot.height,
    left: "50%",
    transform: "translateX(-50%)",
  };
  if (svg) {
    return (
      <div
        className="page"
        data-page={index + 1}
        style={style}
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    );
  }
  return (
    <div className="page" data-page={index + 1} style={style}>
      <div className="tilted-note" aria-hidden="true">
        <span>Rendering…</span>
      </div>
    </div>
  );
}

function Preview({ preview, fetchPages, onViewport, busy, onExternalLink }) {
  const seq = preview?.seq ?? null;
  const pageCount = preview?.pageCount ?? 0;
  const pageSizes = preview?.pageSizes ?? [];
  // Placeholder sheets wear the template's page fill; a transparent or
  // non-solid fill falls back to the UI theme's pane background (§8.6).
  const sheetBg = preview?.pageBackground || "var(--preview-bg)";

  const scrollRef = useRef(null);
  // The scroll window: content-box width, viewport height, scroll offset.
  // Zero until measured — nothing mounts or fetches before that.
  const [view, setView] = useState({ width: 0, height: 0, top: 0 });
  const [cache, setCache] = useState(initialPages);
  const cacheRef = useRef(cache);
  const rafRef = useRef(0);

  const layout = useMemo(() => computeLayout(pageSizes, view.width), [pageSizes, view.width]);
  const { first, last } = visibleRange(layout, view.top, view.height);
  // The topmost visible page (no overscan) for the current/total indicator.
  const currentPage = currentPageAt(layout, view.top, view.height);
  const visible = useMemo(() => {
    const indices = [];
    for (let i = first; i <= last; i++) indices.push(i);
    return indices;
  }, [first, last]);

  // Measure the pane and follow resizes (window, pane split, font scale).
  // The scroll window is also reported up (a ref write in App) so renders
  // can ask for the pages the user is looking at.
  useLayoutEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const measure = () => {
      const vp = { width: el.clientWidth, height: el.clientHeight, top: el.scrollTop };
      setView(vp);
      onViewport?.(vp);
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, [onViewport]);

  // Scroll → one rAF-throttled viewport update (the window computation is
  // cheap; the DOM churn of mounting/unmounting sheets is what we bound).
  const onScroll = useCallback(() => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0;
      const el = scrollRef.current;
      if (!el) return;
      const vp = { width: el.clientWidth, height: el.clientHeight, top: el.scrollTop };
      setView(vp);
      onViewport?.(vp);
    });
  }, [onViewport]);
  useEffect(() => () => cancelAnimationFrame(rafRef.current), []);

  // The mount/fetch lifecycle: prune to the window, fetch what is missing.
  // The reducer deduplicates in-flight indices (also across StrictMode's
  // double effect) and drops responses whose seq or visibility went stale.
  useEffect(() => {
    if (seq == null) return;
    // A new seq may carry the initial window's SVGs (exported with the
    // compile): seed them so the swap is atomic — no placeholder gap.
    const { state, toFetch } = nextViewport(cacheRef.current, seq, visible, preview?.seed);
    cacheRef.current = state;
    setCache(state);
    if (toFetch.length === 0) return;
    fetchPages(toFetch)
      .then((pages) => {
        const merged = applyFetch(cacheRef.current, seq, toFetch, pages);
        cacheRef.current = merged;
        setCache(merged);
      })
      .catch(() => {
        const cleared = failFetch(cacheRef.current, seq, toFetch);
        cacheRef.current = cleared;
        setCache(cleared);
      });
  }, [seq, visible, fetchPages]);

  // Links inside the Typst SVG pages must never navigate the webview away
  // (§9): every `<a>` click is intercepted and handed to the backend, which
  // opens http(s) in the system browser and rejects anything else with an
  // error. Same-document fragment links (`#…`) are internal — e.g. PDF-style
  // footnote/label jumps — and are left to the webview to scroll natively.
  const handleClick = (event) => {
    const anchor = event.target.closest?.("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    if (!href) return;
    if (href.startsWith("#")) return;
    event.preventDefault();
    onExternalLink(href);
  };

  const sheets = [];
  for (let i = first; i <= last; i++) {
    const slot = layout.slots[i];
    if (!slot) continue;
    // Only the current document's markup may mount — between a render
    // landing and the cache resetting, the old SVGs must not flash at the
    // new geometry.
    const svg = cache.seq === seq ? cache.svgs.get(i) : undefined;
    sheets.push(<Sheet key={i} index={i} slot={slot} svg={svg} />);
  }

  return (
    <>
      {busy && (
        <div className="tilted-note" role="status" aria-live="polite">
          <span>Rendering…</span>
        </div>
      )}
      <div
        id="preview"
        ref={scrollRef}
        onScroll={onScroll}
        onClick={handleClick}
        style={{ "--sheet-bg": sheetBg }}
      >
        {pageCount === 0 ? (
          <p className="preview-hint">
            The live preview renders here — per-page SVG from the Typst pipeline.
          </p>
        ) : (
          <div className="preview-spacer" style={{ height: layout.totalHeight }}>
            {sheets}
          </div>
        )}
      </div>
      {currentPage != null && (
        <div className="page-indicator" aria-label={`Page ${currentPage} of ${pageCount}`}>
          {currentPage} / {pageCount}
        </div>
      )}
    </>
  );
}

export default memo(Preview);
