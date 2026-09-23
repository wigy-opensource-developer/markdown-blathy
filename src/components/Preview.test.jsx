// The preview overlay contract (renderStatus drives `busy`): the note shows
// only while a render is pending-and-slow, never replacing the pages — and
// the empty state stays a hint, not an overlay. Rendered through
// react-dom/server (no browser needed).
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import Preview from "./Preview.jsx";

const render = (props) =>
  renderToStaticMarkup(<Preview pages={[]} onExternalLink={() => {}} {...props} />);

describe("Preview", () => {
  it("shows the tilted rendering note while busy", () => {
    const html = render({ pages: ["<svg/>"], busy: true });
    expect(html).toContain("tilted-note");
    expect(html).toContain("Rendering…");
    expect(html).toContain('role="status"');
  });

  it("keeps the note out of the DOM when not busy", () => {
    expect(render({ pages: ["<svg/>"], busy: false })).not.toContain("tilted-note");
  });

  it("renders pages alongside the note (last-good stays visible, dimmed by CSS)", () => {
    const html = render({ pages: ["<svg><rect/></svg>", "<svg/>"], busy: true });
    expect(html).toContain('data-page="1"');
    expect(html).toContain('data-page="2"');
    expect(html).toContain("tilted-note"); // note does not replace the sheets
  });

  it("shows the hint (not pages) while empty, busy or not", () => {
    expect(render({ pages: [], busy: true })).toContain("preview-hint");
    expect(render({ pages: [] })).toContain("preview-hint");
  });
});
