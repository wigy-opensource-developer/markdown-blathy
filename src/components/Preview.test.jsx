// The preview overlay contract (renderStatus drives `busy`): the note shows
// only while a render is pending-and-slow, never replacing the pages — and
// the empty state stays a hint, not an overlay. Rendered through
// react-dom/server (no browser needed): measurement-gated behavior (which
// sheets mount, what is fetched) is pinned by the pure libs
// previewWindow.test.js / previewPages.test.js and the headless harness.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import Preview, { Sheet } from "./Preview.jsx";

const META = { seq: 1, pageCount: 2, pageSizes: [[600, 900], [600, 900]] };
const noop = () => {};
const render = (props) =>
  renderToStaticMarkup(
    <Preview preview={null} fetchPages={async () => []} onExternalLink={noop} {...props} />
  );

describe("Preview", () => {
  it("shows the tilted rendering note while busy", () => {
    const html = render({ preview: META, busy: true });
    expect(html).toContain("tilted-note");
    expect(html).toContain("Rendering…");
    expect(html).toContain('role="status"');
  });

  it("keeps the note out of the DOM when not busy", () => {
    expect(render({ preview: META, busy: false })).not.toContain("tilted-note");
  });

  it("lays out the full-height spacer from metadata alone", () => {
    const html = render({ preview: META });
    expect(html).toContain("preview-spacer");
    // No SVG is mounted before the pane is measured (SSR has no layout):
    // the sheets arrive window-sized in the browser, not in this commit.
    expect(html).not.toContain("data-page");
    expect(html).not.toContain("<svg");
  });

  it("the note coexists with the page list (last-good stays visible, dimmed by CSS)", () => {
    const html = render({ preview: META, busy: true });
    expect(html).toContain("preview-spacer");
    expect(html).toContain("tilted-note"); // note does not replace the sheets
  });

  it("shows the hint (not a page list) while empty, busy or not", () => {
    expect(render({ preview: null, busy: true })).toContain("preview-hint");
    expect(render({ preview: null })).toContain("preview-hint");
    expect(render({ preview: { seq: 1, pageCount: 0, pageSizes: [] } })).toContain(
      "preview-hint"
    );
  });

  it("sheets wear the template's page fill, falling back to the pane background", () => {
    expect(render({ preview: { ...META, pageBackground: "#0d1117" } })).toContain(
      "--sheet-bg:#0d1117"
    );
    expect(render({ preview: META })).toContain("--sheet-bg:var(--preview-bg)");
    expect(render({ preview: { ...META, pageBackground: null } })).toContain(
      "--sheet-bg:var(--preview-bg)"
    );
  });
});

describe("Sheet", () => {
  const slot = { top: 24, width: 400, height: 600 };

  it("a placeholder is a sheet in the paper color with a tilted Rendering note", () => {
    const html = renderToStaticMarkup(<Sheet index={2} slot={slot} svg={undefined} />);
    expect(html).toContain('data-page="3"');
    expect(html).toContain("tilted-note");
    expect(html).toContain("Rendering…");
    // Decorative: the pane-level role="status" note owns the announcement.
    expect(html).toContain('aria-hidden="true"');
    expect(html).not.toContain('role="status"');
    expect(html).not.toContain("<svg");
  });

  it("a mounted sheet injects its SVG and drops the note", () => {
    const html = renderToStaticMarkup(
      <Sheet index={0} slot={slot} svg="<svg><rect/></svg>" />
    );
    expect(html).toContain('data-page="1"');
    expect(html).toContain("<svg><rect/></svg>");
    expect(html).not.toContain("tilted-note");
  });

  it("carries the layout geometry inline", () => {
    const html = renderToStaticMarkup(<Sheet index={0} slot={slot} svg={undefined} />);
    expect(html).toContain("top:24px");
    expect(html).toContain("width:400px");
    expect(html).toContain("height:600px");
  });
});
