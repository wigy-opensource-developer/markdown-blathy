// The toolbar owns file actions, the Document popover (left, with the file
// actions) and the Page settings popover + export (right). The page setup
// selects and template management live inside the Page popover, so the
// toolbar only carries a dot marking an active custom template. Rendered
// through react-dom/server.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import Toolbar from "./Toolbar.jsx";

const render = (props) =>
  renderToStaticMarkup(
    <MantineProvider>
      <Toolbar
        options={{ templatePath: null }}
        onOptionChange={() => {}}
        onNew={() => {}}
        onOpen={() => {}}
        onSave={() => {}}
        onPickTemplate={() => {}}
        onClearTemplate={() => {}}
        onExport={() => {}}
        docMeta={null}
        onApplySettings={() => {}}
        onSettingsOpenChange={() => {}}
        {...props}
      />
    </MantineProvider>
  );

describe("Toolbar", () => {
  it("does not render the paper, margins or align selects", () => {
    const html = render();
    expect(html).not.toContain("Paper");
    expect(html).not.toContain("Margins");
    expect(html).not.toContain("Align");
  });

  it("keeps the file actions and export", () => {
    const html = render();
    expect(html).toContain("New");
    expect(html).toContain("Open");
    expect(html).toContain("Save");
    expect(html).toContain("Export PDF");
  });

  it("offers Page settings on the right and no template controls", () => {
    const html = render();
    expect(html).toContain("Page…");
    expect(html).not.toContain("Template…");
    expect(html).not.toContain("Custom template");
  });

  it("marks the Page button only while a custom template is active", () => {
    expect(render()).not.toContain("template-dot");
    const html = render({ options: { templatePath: "/home/u/dark.typ" } });
    expect(html).toContain("template-dot");
    expect(html).toContain("Template: dark.typ");
  });
});
