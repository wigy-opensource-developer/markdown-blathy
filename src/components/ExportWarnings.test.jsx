// The post-export warning panel: names the written PDF, lists the compile
// warnings and offers an explicit dismiss. Rendered through react-dom/server.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import ExportWarnings from "./ExportWarnings.jsx";

const NOTICE = {
  path: "/home/u/report.pdf",
  warnings: ["duplicate heading ID \"intro\"", "unknown font: Fira Sans"],
};

describe("ExportWarnings", () => {
  it("renders nothing without a notice", () => {
    expect(renderToStaticMarkup(<ExportWarnings notice={null} onClose={() => {}} />)).toBe("");
  });

  it("names the exported file, counts and lists the warnings", () => {
    const html = renderToStaticMarkup(
      <ExportWarnings notice={NOTICE} onClose={() => {}} />
    );
    expect(html).toContain("report.pdf");
    expect(html).toContain("2 warnings");
    expect(html).toContain("duplicate heading ID");
    expect(html).toContain("unknown font: Fira Sans");
  });

  it("offers an explicit dismiss button", () => {
    const html = renderToStaticMarkup(
      <ExportWarnings notice={NOTICE} onClose={() => {}} />
    );
    expect(html).toContain('aria-label="Dismiss export warnings"');
  });
});
