// The crash-recovery prompt body: which document, when, and the two choices.
// Rendered through react-dom/server; the portal-based Modal is not mountable
// there, which is why the body is its own component.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { MantineProvider } from "@mantine/core";
import { RecoveryPromptBody } from "./RecoveryPrompt.jsx";

const SNAPSHOT = {
  id: "0123456789abcdef",
  markdown: "# Draft",
  path: "/home/u/report.md",
  savedAt: 1_760_000_000,
};

const render = (snapshot = SNAPSHOT) =>
  renderToStaticMarkup(
    <MantineProvider>
      <RecoveryPromptBody
        snapshot={snapshot}
        onRestore={() => {}}
        onDiscard={() => {}}
      />
    </MantineProvider>
  );

describe("RecoveryPromptBody", () => {
  it("names the document and offers Restore and Discard", () => {
    const html = render();
    expect(html).toContain("report.md");
    expect(html).toContain("Restore");
    expect(html).toContain("Discard");
  });

  it("labels an untitled snapshot", () => {
    expect(render({ ...SNAPSHOT, path: null })).toContain("Untitled");
  });

  it("says restoring keeps the document unsaved", () => {
    expect(render()).toContain("marks the document as unsaved");
  });
});
