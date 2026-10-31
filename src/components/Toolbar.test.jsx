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
        platform="linux"
        dispatch={() => true}
        options={{ templatePath: null }}
        theme="system"
        onThemeChange={() => {}}
        onOptionChange={() => {}}
        onPickTemplate={() => {}}
        onClearTemplate={() => {}}
        docMeta={null}
        onApplySettings={() => {}}
        documentSettingsOpen={false}
        onDocumentSettingsOpenChange={() => {}}
        pageSettingsOpen={false}
        onPageSettingsOpenChange={() => {}}
        themeMenuOpen={false}
        onThemeMenuOpenChange={() => {}}
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

  it("wires the file actions and export to command ids", () => {
    const html = render();
    expect(html).toContain("New");
    expect(html).toContain("Open");
    expect(html).toContain("Save");
    expect(html).toContain("Export PDF");
    for (const id of ["file.new", "file.open", "file.save", "file.exportPdf"]) {
      expect(html).toContain(`data-command="${id}"`);
    }
  });

  it("exposes the registry shortcuts through aria-keyshortcuts", () => {
    const html = render();
    expect(html).toContain('aria-keyshortcuts="Control+N"');
    expect(html).toContain('aria-keyshortcuts="Control+O"');
    expect(html).toContain('aria-keyshortcuts="Control+S"');
    expect(html).toContain('aria-keyshortcuts="Control+E"');
    expect(html).toContain('aria-keyshortcuts="Control+,"');
    expect(html).toContain('aria-keyshortcuts="Control+Shift+,"');
  });

  it("uses the macOS modifier in aria-keyshortcuts", () => {
    const html = render({ platform: "mac" });
    expect(html).toContain('aria-keyshortcuts="Meta+S"');
  });

  it("offers Page settings on the right and no template controls", () => {
    const html = render();
    expect(html).toContain("Page…");
    expect(html).toContain('data-command="page.settings"');
    expect(html).toContain('data-command="document.settings"');
    expect(html).not.toContain("Template…");
    expect(html).not.toContain("Custom template");
  });

  it("shows the theme icon button labelled with the current theme", () => {
    const html = render();
    expect(html).toContain('aria-label="Color theme: System"');
    expect(html).toContain("<svg"); // the sun/moon icon
  });

  it("marks the Page button only while a custom template is active", () => {
    expect(render()).not.toContain("template-dot");
    const html = render({ options: { templatePath: "/home/u/dark.typ" } });
    expect(html).toContain("template-dot");
    expect(html).toContain("Template: dark.typ");
  });
});
