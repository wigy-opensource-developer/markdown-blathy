// The unsaved-changes prompt body: the three choices and the busy state.
// Rendered through react-dom/server; the portal-based Modal wrapper is not
// mountable there, which is why the body is its own component.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { UnsavedChangesBody } from "./UnsavedChangesModal.jsx";

const render = (props) =>
  renderToStaticMarkup(
    <MantineProvider>
      <UnsavedChangesBody onChoose={() => {}} {...props} />
    </MantineProvider>
  );

describe("UnsavedChangesBody", () => {
  it("explains the situation and offers Save, Discard and Cancel", () => {
    const html = render();
    expect(html).toContain("unsaved changes");
    expect(html).toContain("Save");
    expect(html).toContain("Discard");
    expect(html).toContain("Cancel");
  });

  it("disables every choice while a save is in flight", () => {
    const html = render({ busy: true });
    expect(html.match(/disabled/g)?.length ?? 0).toBeGreaterThanOrEqual(2);
  });
});
