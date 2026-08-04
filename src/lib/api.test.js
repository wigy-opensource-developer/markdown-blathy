// IPC contract tests for the Tauri command wrappers (§8.4 of SPEC.md).
//
// `@tauri-apps/api/core` is mocked so the tests run in plain vitest; they pin
// the exact command names and argument shapes the Rust commands deserialize
// (mirror of `commands.rs`'s render_payload_deserializes_frontend_shape).
import { describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import {
  exportPdf,
  isCanceled,
  openMarkdown,
  renderPreview,
  saveMarkdown,
} from "./api.js";
import { renderOptionsPayload } from "./presets.js";

const OPTIONS = {
  flavor: "gfm",
  paper: "a4",
  marginPreset: "normal",
  margins: { topMm: 25, bottomMm: 25, leftMm: 20, rightMm: 20 },
  textAlign: "left",
  templatePath: null,
};

describe("renderPreview", () => {
  it("invokes render_preview with the camelCase payload and echoes seq", () => {
    renderPreview("# Hi", OPTIONS, 7);
    expect(invoke).toHaveBeenCalledWith("render_preview", {
      payload: { markdown: "# Hi", options: renderOptionsPayload(OPTIONS), seq: 7 },
    });
  });
});

describe("openMarkdown", () => {
  it("invokes open_markdown with no arguments", () => {
    openMarkdown();
    expect(invoke).toHaveBeenCalledWith("open_markdown");
  });
});

describe("saveMarkdown", () => {
  it("invokes save_markdown with path and content", () => {
    saveMarkdown("/tmp/doc.md", "# Hi");
    expect(invoke).toHaveBeenCalledWith("save_markdown", {
      path: "/tmp/doc.md",
      content: "# Hi",
    });
  });

  it("passes null as the path for an untitled document", () => {
    saveMarkdown(null, "# Hi");
    expect(invoke).toHaveBeenCalledWith("save_markdown", {
      path: null,
      content: "# Hi",
    });
  });
});

describe("exportPdf", () => {
  it("invokes export_pdf with the render payload (seq unused, 0)", () => {
    exportPdf("# Hi", OPTIONS);
    expect(invoke).toHaveBeenCalledWith("export_pdf", {
      payload: { markdown: "# Hi", options: renderOptionsPayload(OPTIONS), seq: 0 },
    });
  });
});

describe("isCanceled", () => {
  it("is true only when the result carries canceled: true (§9)", () => {
    expect(isCanceled({ canceled: true })).toBe(true);
    expect(isCanceled({ path: "/a.md", content: "# Hi" })).toBe(false);
    expect(isCanceled({ path: "/a.pdf" })).toBe(false);
    expect(isCanceled(undefined)).toBe(false);
  });
});
