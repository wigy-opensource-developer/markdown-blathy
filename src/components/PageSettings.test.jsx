// The Page settings popover body: global page-setup preferences (paper,
// margins, alignment) plus the Typst template override. Unlike the Document
// popover these are not per-document, so the scope line matters; the two
// reset paths (template only, all defaults) are pinned here. Rendered through
// react-dom/server (no browser needed).
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { PageSettingsForm } from "./PageSettings.jsx";

const OPTIONS = {
  paper: "a5",
  marginPreset: "narrow",
  margins: { topMm: 15, bottomMm: 15, leftMm: 10, rightMm: 10 },
  textAlign: "justified",
  templatePath: null,
};

const render = (props) =>
  renderToStaticMarkup(
    <MantineProvider>
      <PageSettingsForm
        options={OPTIONS}
        onOptionChange={() => {}}
        onPickTemplate={() => {}}
        onClearTemplate={() => {}}
        {...props}
      />
    </MantineProvider>
  );

describe("PageSettingsForm", () => {
  it("states that these settings apply to every document", () => {
    expect(render()).toContain("Applies to all documents on this device");
  });

  it("shows the current page setup values", () => {
    const html = render();
    expect(html).toContain('value="A5"');
    expect(html).toContain('value="Narrow (10/15mm)"');
    expect(html).toContain('value="Justified"');
  });

  it("shows the embedded default and no reset when no custom template is active", () => {
    const html = render();
    expect(html).toContain("Embedded default");
    expect(html).not.toContain("Reset custom template");
  });

  it("shows the template basename and a reset control when one is active", () => {
    const html = render({
      options: { ...OPTIONS, templatePath: "/home/u/dark.typ" },
    });
    expect(html).toContain("dark.typ");
    expect(html).toContain("Reset custom template");
  });

  it("offers restoring the page-setup defaults", () => {
    expect(render()).toContain("Restore defaults");
  });
});
