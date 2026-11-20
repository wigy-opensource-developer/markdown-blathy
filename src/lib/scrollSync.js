// Bidirectional editor ↔ preview scroll synchronization.
//
// The backend resolves invisible metadata on rendered leaf blocks to
// { startLine, page, y } (one-based line/page; y in Typst points). Here those
// points become CSS scroll coordinates using the SAME virtual page geometry
// as Preview.jsx, so the target page need not have its SVG mounted.
//
// Focus invariant: this controller ONLY writes scrollTop. It never calls
// focus()/scrollIntoView() or dispatches transactions, so synchronized
// scrolling cannot move keyboard focus or the caret — typing continues
// uninterrupted in whichever pane owns it.

import { computeLayout, PAGE_PAD } from "./previewWindow.js";

const FOCAL_RATIO = 0.25;
const SCROLL_EPSILON = 1;

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function anchorY(anchor, layout) {
  const slot = layout.slots[anchor.page - 1];
  const size = layout.pageSizes?.[anchor.page - 1];
  if (!slot || !size || !(size[1] > 0)) return null;
  const y = slot.top + (anchor.y / size[1]) * slot.height;
  return Number.isFinite(y) ? y : null;
}

/** Build the strictly source-line-ordered interpolation knots, including
 * document boundaries so the editor's first/last lines can reach the ends of
 * long final blocks. Invalid, duplicate, or physically backwards anchors are
 * skipped defensively; the converter normally guarantees none of them.
 *
 * Cached per anchors array + geometry: scroll frames call the mapping 60×/s
 * and block-heavy documents have thousands of anchors — sorting them per
 * frame would be O(n log n) each time. The slots are fully determined by the
 * pageSizes array and the scaled page width, so those plus the line count
 * form the cache key. */
const knotCache = new WeakMap();

function knots(anchors, layout, sourceLineCount) {
  if (!Array.isArray(anchors) || anchors.length === 0 || layout.slots.length === 0) {
    return [];
  }
  const sizes = layout.pageSizes ?? [];
  const width = layout.slots[0].width;
  const cached = knotCache.get(anchors);
  if (cached && cached.sizes === sizes && cached.width === width && cached.lines === sourceLineCount) {
    return cached.points;
  }
  const result = [{ line: 1, y: PAGE_PAD }];
  let lastLine = 1;
  let lastY = PAGE_PAD;
  const sorted = [...anchors].sort((a, b) => a.startLine - b.startLine);
  for (const anchor of sorted) {
    if (
      !Number.isInteger(anchor.startLine) ||
      anchor.startLine <= lastLine ||
      anchor.startLine > sourceLineCount
    ) {
      continue;
    }
    const y = anchorY(anchor, layout);
    if (y == null || y < lastY) continue;
    result.push({ line: anchor.startLine, y });
    lastLine = anchor.startLine;
    lastY = y;
  }
  if (sourceLineCount > lastLine) {
    const y = Math.max(lastY, layout.totalHeight - PAGE_PAD);
    result.push({ line: sourceLineCount, y });
  }
  knotCache.set(anchors, { sizes, width, lines: sourceLineCount, points: result });
  return result;
}

/** First index i ≥ 1 with key(points[i]) ≥ value (points sorted by key). */
function upperSegment(points, value, key) {
  let lo = 1;
  let hi = points.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (key(points[mid]) < value) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** Map a (possibly fractional) one-based Markdown line to the virtual
 * preview's document-level CSS Y coordinate. Null means no usable map. */
export function previewYAtSourceLine(anchors, line, layout, sourceLineCount) {
  const points = knots(anchors, layout, sourceLineCount);
  if (points.length === 0) return null;
  if (points.length === 1 || line <= points[0].line) return points[0].y;
  const last = points[points.length - 1];
  if (line >= last.line) return last.y;
  const i = upperSegment(points, line, (p) => p.line);
  const left = points[i - 1];
  const right = points[i];
  const t = clamp((line - left.line) / (right.line - left.line), 0, 1);
  return left.y + (right.y - left.y) * t;
}

/** Inverse of previewYAtSourceLine, using the same knots. Equal-Y anchors
 * collapse by choosing the later source segment, avoiding division by zero. */
export function sourceLineAtPreviewY(anchors, y, layout, sourceLineCount) {
  const points = knots(anchors, layout, sourceLineCount);
  if (points.length === 0) return null;
  if (points.length === 1 || y <= points[0].y) return points[0].line;
  const last = points[points.length - 1];
  if (y >= last.y) return last.line;
  const i = upperSegment(points, y, (p) => p.y);
  const left = points[i - 1];
  const right = points[i];
  if (right.y === left.y) return right.line;
  const t = clamp((y - left.y) / (right.y - left.y), 0, 1);
  return left.line + (right.line - left.line) * t;
}

function previewLayout(preview, scroller) {
  const layout = computeLayout(preview?.pageSizes ?? [], scroller.clientWidth);
  // anchorY uses the intrinsic sizes to convert points; retaining them next
  // to the slots avoids making every pure mapping call pass them separately.
  layout.pageSizes = preview?.pageSizes ?? [];
  return layout;
}

/** Create the stateful DOM controller. Its getters keep all frequently
 * changing values in refs: a scroll never needs a React commit. */
export function createScrollSyncController({
  getEditorView,
  getPreviewScroller,
  getPreview,
  isPreviewCurrent,
  requestFrame = requestAnimationFrame,
  cancelFrame = cancelAnimationFrame,
}) {
  let enabled = true;
  let owner = "editor";
  let queued = null;
  let frame = 0;
  const expected = { editor: null, preview: null };

  function currentMapping() {
    if (!enabled || !isPreviewCurrent()) return null;
    const editor = getEditorView();
    const previewScroller = getPreviewScroller();
    const preview = getPreview();
    if (!editor || !previewScroller || !preview?.sourceAnchors?.length) return null;
    const layout = previewLayout(preview, previewScroller);
    if (layout.slots.length === 0) return null;
    return { editor, previewScroller, preview, layout };
  }

  function write(pane, scroller, top) {
    const maximum = Math.max(0, scroller.scrollHeight - scroller.clientHeight);
    const next = clamp(top, 0, maximum);
    expected[pane] = next;
    scroller.scrollTop = next;
    // No event is generated when the value did not actually change.
    if (Math.abs(scroller.scrollTop - next) > SCROLL_EPSILON) expected[pane] = null;
  }

  function synchronize(source) {
    const state = currentMapping();
    if (!state) return;
    const { editor, previewScroller, preview, layout } = state;
    const sourceLines = editor.state.doc.lines;
    if (source === "editor") {
      const scroller = editor.scrollDOM;
      const focal = scroller.scrollTop + scroller.clientHeight * FOCAL_RATIO;
      const block = editor.lineBlockAtHeight(focal);
      const line = editor.state.doc.lineAt(block.from).number;
      const fraction = block.height > 0 ? clamp((focal - block.top) / block.height, 0, 1) : 0;
      const y = previewYAtSourceLine(
        preview.sourceAnchors,
        line + fraction,
        layout,
        sourceLines
      );
      if (y != null) {
        write("preview", previewScroller, y - previewScroller.clientHeight * FOCAL_RATIO);
      }
    } else {
      const y = previewScroller.scrollTop + previewScroller.clientHeight * FOCAL_RATIO;
      const line = sourceLineAtPreviewY(preview.sourceAnchors, y, layout, sourceLines);
      if (line == null) return;
      const number = clamp(Math.round(line), 1, sourceLines);
      const block = editor.lineBlockAt(editor.state.doc.line(number).from);
      write(
        "editor",
        editor.scrollDOM,
        block.top - editor.scrollDOM.clientHeight * FOCAL_RATIO
      );
    }
  }

  function onScroll(source) {
    if (!enabled) return;
    const scroller = source === "editor" ? getEditorView()?.scrollDOM : getPreviewScroller();
    if (!scroller) return;
    if (expected[source] != null) {
      if (Math.abs(scroller.scrollTop - expected[source]) <= SCROLL_EPSILON) {
        expected[source] = null;
        return;
      }
      // A different position means a real user scroll superseded our write.
      expected[source] = null;
    }
    owner = source;
    queued = source;
    if (frame) return;
    frame = requestFrame(() => {
      frame = 0;
      const next = queued;
      queued = null;
      if (next) synchronize(next);
    });
  }

  function resynchronize() {
    if (!enabled || frame) return;
    queued = owner;
    frame = requestFrame(() => {
      frame = 0;
      const next = queued;
      queued = null;
      if (next) synchronize(next);
    });
  }

  function setEnabled(next) {
    enabled = Boolean(next);
    expected.editor = null;
    expected.preview = null;
    if (!enabled && frame) {
      cancelFrame(frame);
      frame = 0;
      queued = null;
    } else if (enabled) {
      resynchronize();
    }
  }

  function reset() {
    owner = "editor";
    queued = null;
    expected.editor = null;
    expected.preview = null;
    const editor = getEditorView();
    const preview = getPreviewScroller();
    if (editor) editor.scrollDOM.scrollTop = 0;
    if (preview) preview.scrollTop = 0;
  }

  function dispose() {
    if (frame) cancelFrame(frame);
    frame = 0;
    queued = null;
  }

  return { onScroll, resynchronize, setEnabled, reset, dispose };
}
