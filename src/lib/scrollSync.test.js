import { describe, expect, it } from "vitest";
import {
  createScrollSyncController,
  previewYAtSourceLine,
  sourceLineAtPreviewY,
} from "./scrollSync.js";
import { computeLayout } from "./previewWindow.js";

function layout() {
  const result = computeLayout(
    [
      [100, 200],
      [100, 200],
    ],
    100
  );
  result.pageSizes = [
    [100, 200],
    [100, 200],
  ];
  return result;
}

const anchors = [
  { startLine: 2, page: 1, y: 50 },
  { startLine: 5, page: 2, y: 100 },
];

describe("scroll source interpolation", () => {
  it("is linear between anchors and invertible", () => {
    const y = previewYAtSourceLine(anchors, 3.5, layout(), 10);
    expect(y).toBe(211); // line 2 → 74; line 5 → 348
    expect(sourceLineAtPreviewY(anchors, y, layout(), 10)).toBeCloseTo(3.5);
  });

  it("uses document boundaries instead of extrapolating", () => {
    expect(previewYAtSourceLine(anchors, 1, layout(), 10)).toBe(24);
    expect(previewYAtSourceLine(anchors, 10, layout(), 10)).toBe(448);
    expect(sourceLineAtPreviewY(anchors, 0, layout(), 10)).toBe(1);
    expect(sourceLineAtPreviewY(anchors, 9999, layout(), 10)).toBe(10);
  });

  it("skips duplicate lines and non-monotonic physical anchors", () => {
    const input = [
      { startLine: 1, page: 1, y: 100 },
      { startLine: 2, page: 1, y: 50 },
      { startLine: 2, page: 1, y: 80 },
      { startLine: 3, page: 1, y: 40 },
    ];
    expect(previewYAtSourceLine(input, 2, layout(), 4)).toBe(74);
    expect(previewYAtSourceLine(input, 3, layout(), 4)).toBe(261);
  });

  it("reports no map for an empty or unmeasured preview", () => {
    expect(previewYAtSourceLine([], 1, layout(), 1)).toBeNull();
    expect(
      previewYAtSourceLine(anchors, 1, { slots: [], totalHeight: 0, pageSizes: [] }, 10)
    ).toBeNull();
  });

  it("recomputes knots when the pane geometry changes (cache key)", () => {
    const sizes = [
      [100, 200],
      [100, 200],
    ];
    const wide = computeLayout(sizes, 100);
    wide.pageSizes = sizes;
    const narrow = computeLayout(sizes, 50);
    narrow.pageSizes = sizes;
    const yWide = previewYAtSourceLine(anchors, 3.5, wide, 10);
    const yNarrow = previewYAtSourceLine(anchors, 3.5, narrow, 10);
    expect(yWide).not.toBe(yNarrow);
    // Same geometry again: identical result (cached path).
    expect(previewYAtSourceLine(anchors, 3.5, wide, 10)).toBe(yWide);
    expect(sourceLineAtPreviewY(anchors, yNarrow, narrow, 10)).toBeCloseTo(3.5);
  });

  it("interpolates correctly across many anchors (binary search)", () => {
    const many = [];
    for (let i = 0; i < 500; i++) {
      many.push({ startLine: 2 + i * 2, page: 1, y: 10 + i });
    }
    const l = layout();
    // Lines 500 and 502 are consecutive anchors; 501 is their midpoint.
    const y = previewYAtSourceLine(many, 501, l, 1200);
    const y500 = previewYAtSourceLine(many, 500, l, 1200);
    const y502 = previewYAtSourceLine(many, 502, l, 1200);
    expect(y).toBeCloseTo((y500 + y502) / 2, 6);
    expect(sourceLineAtPreviewY(many, y, l, 1200)).toBeCloseTo(501, 6);
  });
});

function fakeEditor() {
  const scroller = { scrollTop: 0, scrollHeight: 220, clientHeight: 100 };
  const block = (number) => ({
    from: (number - 1) * 10,
    top: (number - 1) * 20,
    height: 20,
  });
  const focusCalls = { view: 0, content: 0 };
  return {
    scrollDOM: scroller,
    focusCalls,
    focus() {
      focusCalls.view++;
    },
    contentDOM: {
      focus() {
        focusCalls.content++;
      },
    },
    state: {
      doc: {
        lines: 10,
        lineAt(from) {
          return { number: Math.floor(from / 10) + 1 };
        },
        line(number) {
          return { from: (number - 1) * 10 };
        },
      },
    },
    lineBlockAtHeight(height) {
      return block(Math.min(10, Math.max(1, Math.floor(height / 20) + 1)));
    },
    lineBlockAt(position) {
      return block(Math.floor(position / 10) + 1);
    },
  };
}

function controllerFixture() {
  const editor = fakeEditor();
  const previewScroller = {
    scrollTop: 0,
    scrollHeight: 572,
    clientHeight: 100,
    clientWidth: 100,
  };
  const preview = {
    pageSizes: [
      [100, 200],
      [100, 200],
    ],
    sourceAnchors: anchors,
  };
  let current = true;
  let nextFrame = 1;
  const callbacks = new Map();
  const controller = createScrollSyncController({
    getEditorView: () => editor,
    getPreviewScroller: () => previewScroller,
    getPreview: () => preview,
    isPreviewCurrent: () => current,
    requestFrame(callback) {
      const id = nextFrame++;
      callbacks.set(id, callback);
      return id;
    },
    cancelFrame(id) {
      callbacks.delete(id);
    },
  });
  return {
    editor,
    previewScroller,
    controller,
    setCurrent(value) {
      current = value;
    },
    flush() {
      const pending = [...callbacks.values()];
      callbacks.clear();
      for (const callback of pending) callback();
    },
  };
}

describe("scroll sync controller", () => {
  it("synchronizes editor to preview without a feedback write", () => {
    const f = controllerFixture();
    f.editor.scrollDOM.scrollTop = 100;
    f.controller.onScroll("editor");
    f.flush();
    const target = f.previewScroller.scrollTop;
    expect(target).toBeGreaterThan(0);
    const editorTop = f.editor.scrollDOM.scrollTop;
    f.controller.onScroll("preview"); // event caused by the programmatic write
    f.flush();
    expect(f.editor.scrollDOM.scrollTop).toBe(editorTop);
  });

  it("allows the preview to become the authoritative side", () => {
    const f = controllerFixture();
    f.previewScroller.scrollTop = 200;
    f.controller.onScroll("preview");
    f.flush();
    expect(f.editor.scrollDOM.scrollTop).toBeGreaterThan(0);
  });

  it("never moves keyboard focus while synchronizing", () => {
    const f = controllerFixture();
    f.editor.scrollDOM.scrollTop = 100;
    f.controller.onScroll("editor");
    f.flush();
    f.previewScroller.scrollTop = 250;
    f.controller.onScroll("preview");
    f.flush();
    f.controller.resynchronize();
    f.flush();
    expect(f.editor.focusCalls).toEqual({ view: 0, content: 0 });
  });

  it("pauses on stale maps and while disabled", () => {
    const f = controllerFixture();
    f.setCurrent(false);
    f.editor.scrollDOM.scrollTop = 100;
    f.controller.onScroll("editor");
    f.flush();
    expect(f.previewScroller.scrollTop).toBe(0);
    f.setCurrent(true);
    f.controller.setEnabled(false);
    f.controller.onScroll("editor");
    f.flush();
    expect(f.previewScroller.scrollTop).toBe(0);
  });
});
