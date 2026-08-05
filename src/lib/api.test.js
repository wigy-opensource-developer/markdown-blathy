// IPC contract tests for the Tauri command wrappers (§8.4 of SPEC.md).
//
// `@tauri-apps/api/core` is mocked so the tests run in plain vitest; they pin
// the exact command names and argument shapes the Rust commands deserialize
// (mirror of `commands.rs`'s render_payload_deserializes_frontend_shape).
import { describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const { getCurrentWindow } = vi.hoisted(() => {
  const setTitle = vi.fn();
  return { getCurrentWindow: vi.fn(() => ({ setTitle })) };
});
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow }));

import {
  exportPdf,
  getInitialDirectory,
  isCanceled,
  openExternal,
  openMarkdown,
  renderPreview,
  saveMarkdown,
  setWindowTitle,
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
  it("invokes open_markdown with the directory to start browsing in", () => {
    openMarkdown("/home/u/Documents");
    expect(invoke).toHaveBeenCalledWith("open_markdown", {
      directory: "/home/u/Documents",
    });
  });

  it("passes null directory before the initial directory resolves", () => {
    openMarkdown(null);
    expect(invoke).toHaveBeenCalledWith("open_markdown", { directory: null });
  });
});

describe("saveMarkdown", () => {
  it("invokes save_markdown with content, sanitized title and directory", () => {
    saveMarkdown("# Hi", "My Doc", "/home/u/Documents");
    expect(invoke).toHaveBeenCalledWith("save_markdown", {
      content: "# Hi",
      title: "My Doc",
      directory: "/home/u/Documents",
    });
  });
});

describe("exportPdf", () => {
  it("invokes export_pdf with the render payload, title and directory", () => {
    exportPdf("# Hi", OPTIONS, "My Doc", "/home/u/Documents");
    expect(invoke).toHaveBeenCalledWith("export_pdf", {
      payload: { markdown: "# Hi", options: renderOptionsPayload(OPTIONS), seq: 0 },
      title: "My Doc",
      directory: "/home/u/Documents",
    });
  });
});

describe("getInitialDirectory", () => {
  it("invokes get_initial_directory with no arguments", () => {
    getInitialDirectory();
    expect(invoke).toHaveBeenCalledWith("get_initial_directory");
  });
});

describe("setWindowTitle", () => {
  it("sets the OS window title via the Tauri window API", () => {
    setWindowTitle("Sample - Markdown to PDF");
    expect(getCurrentWindow().setTitle).toHaveBeenCalledWith(
      "Sample - Markdown to PDF"
    );
  });
});

describe("openExternal", () => {
  it("invokes open_external with the url", () => {
    openExternal("https://example.com");
    expect(invoke).toHaveBeenCalledWith("open_external", {
      url: "https://example.com",
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
