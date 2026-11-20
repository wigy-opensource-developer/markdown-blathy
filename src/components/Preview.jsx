// Preview pane: a virtualized list of "sheets", one per printed page.
//
// Pages are PNG rasters of the same paged document the PDF uses (typst-render),
// shown as data-URL <img>s. Image insertion is near-free and the browser decodes
// asynchronously off the main thread, so scrolling never blocks the UI thread.
// An <img> has no clickable DOM, so the backend's link rects are overlaid as
// transparent anchors (SPEC §9). Every exported page stays mounted for the life
// of the render (keep-all cache), so scrolling back needs no IPC and no re-decode.
//
// Why PNG and not SVG: see IMPLEMENTATION.md, "Preview rendering: PNG rasters".
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { computeLayout, visibleRange, currentPageAt } from "../lib/previewWindow.js";
import { initialPages, nextViewport, applyFetch, failFetch } from "../lib/previewPages.js";

/** One sheet: the printed page (PNG <img> + link overlays) or, before its
 *  raster arrives, a placeholder in the template's paper color (--sheet-bg)
 *  carrying the tilted "Rendering…" note. The note is aria-hidden — the
 *  pane-level role="status" note owns the announcement. */
export function Sheet({ index, slot, page, pageWidth }) {
  const style = {
    top: slot.top,
    width: slot.width,
    height: slot.height,
    left: "50%",
    transform: "translateX(-50%)",
  };
  const png = page?.png;
  // Link rects are page-local points; scale them into the sheet's CSS box.
  const scale = pageWidth > 0 ? slot.width / pageWidth : 0;
  if (png) {
    return (
      <div className="page" data-page={index + 1} style={style}>
        <img
          className="page-img"
          src={`data:image/png;base64,${png}`}
          alt={`Page ${index + 1}`}
          draggable={false}
        />
        {page.links?.map((l, i) => (
          <a
            key={i}
            className="page-link"
            href={l.href}
            draggable={false}
            style={{ left: l.x * scale, top: l.y * scale, width: l.w * scale, height: l.h * scale }}
          />
        ))}
      </div>
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

function Preview({
  preview,
  fetchPages,
  onViewport,
  onScroller,
  onSyncScroll,
  busy,
  onExternalLink,
}) {
  const seq = preview?.seq ?? null;
  const pageCount = preview?.pageCount ?? 0;
  const pageSizes = preview?.pageSizes ?? [];
  // Placeholder sheets wear the template's page fill; a transparent or
  // non-solid fill falls back to the UI theme's pane background (§8.6).
  const sheetBg = preview?.pageBackground || "var(--preview-bg)";

  const scrollRef = useRef(null);
  useEffect(() => {
    onScroller?.(scrollRef.current);
    return () => onScroller?.(null);
  }, [onScroller]);
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

  // Scroll → one rAF-throttled viewport update.
  const onScroll = useCallback(() => {
    onSyncScroll?.();
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0;
      const el = scrollRef.current;
      if (!el) return;
      const vp = { width: el.clientWidth, height: el.clientHeight, top: el.scrollTop };
      setView(vp);
      onViewport?.(vp);
    });
  }, [onViewport, onSyncScroll]);
  useEffect(() => () => cancelAnimationFrame(rafRef.current), []);

  // Fetch the visible window's missing pages (top-first). The reducer keeps
  // every page held for this seq, deduplicates in-flight indices (also across
  // StrictMode's double effect), and drops responses whose seq went stale.
  useEffect(() => {
    if (seq == null) return;
    // A new seq may carry the initial window's rasters (exported with the
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

  // Link overlays must never navigate the webview away (§9): every <a> click
  // is intercepted and handed to the backend, which opens http(s) in the
  // system browser and rejects other schemes. Fragment links (#…) are internal
  // and left to the webview (inert until typst resolves paged anchors).
  const handleClick = (event) => {
    const anchor = event.target.closest?.("a");
    if (!anchor) return;
    const href = anchor.getAttribute("href");
    if (!href) return;
    if (href.startsWith("#")) return;
    event.preventDefault();
    onExternalLink(href);
  };

  // Render every cached page (kept mounted even off-screen) plus the visible
  // window's placeholders. The cache.seq guard keeps a superseded document's
  // pages from flashing at the new geometry before the cache resets.
  const indices = new Set(cache.seq === seq ? cache.pages.keys() : []);
  for (let i = first; i <= last; i++) indices.add(i);
  const sheets = [];
  for (const i of [...indices].sort((a, b) => a - b)) {
    const slot = layout.slots[i];
    if (!slot) continue;
    const page = cache.seq === seq ? cache.pages.get(i) : undefined;
    sheets.push(
      <Sheet key={i} index={i} slot={slot} page={page} pageWidth={pageSizes[i]?.[0] ?? 0} />
    );
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
            The live preview renders here — per-page rasters from the Typst pipeline.
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
