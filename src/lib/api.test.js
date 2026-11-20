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
  clearRecovery,
  confirmClose,
  discardRecovery,
  exportPdf,
  exportPages,
  forgetDocument,
  frontendReady,
  getInitialDirectory,
  isCanceled,
  listTemplates,
  openExternal,
  openMarkdown,
  openPendingDocument,
  renderPreview,
  restoreRecovery,
  saveDocument,
  saveMarkdown,
  saveRecoverySnapshot,
  setWindowTitle,
  takeRecovery,
  updateFrontMatter,
  validateTemplate,
} from "./api.js";
import { renderOptionsPayload } from "./presets.js";

const OPTIONS = {
  flavor: "github",
  paper: "a4",
  marginPreset: "normal",
  margins: { topMm: 25, bottomMm: 25, leftMm: 20, rightMm: 20 },
  textAlign: "left",
  templateName: null,
  templatePath: null,
};

describe("renderPreview", () => {
  it("invokes render_preview with the camelCase payload and echoes seq", () => {
    renderPreview("# Hi", OPTIONS, 7);
    expect(invoke).toHaveBeenCalledWith("render_preview", {
      payload: {
        markdown: "# Hi",
        options: renderOptionsPayload(OPTIONS),
        seq: 7,
        documentPath: null,
        pages: [],
        pixelWidth: null,
      },
    });
  });

  it("forwards the document path for document-relative images", () => {
    renderPreview("# Hi", OPTIONS, 8, "/home/u/docs/report.md");
    expect(invoke).toHaveBeenCalledWith("render_preview", {
      payload: {
        markdown: "# Hi",
        options: renderOptionsPayload(OPTIONS),
        seq: 8,
        documentPath: "/home/u/docs/report.md",
        pages: [],
        pixelWidth: null,
      },
    });
  });

  it("forwards the initial page window for the atomic swap", () => {
    renderPreview("# Hi", OPTIONS, 9, null, [3, 4, 5]);
    expect(invoke).toHaveBeenCalledWith("render_preview", {
      payload: {
        markdown: "# Hi",
        options: renderOptionsPayload(OPTIONS),
        seq: 9,
        documentPath: null,
        pages: [3, 4, 5],
        pixelWidth: null,
      },
    });
  });

  it("forwards the target raster pixel width", () => {
    renderPreview("# Hi", OPTIONS, 10, null, [0], 1640);
    expect(invoke).toHaveBeenCalledWith(
      "render_preview",
      expect.objectContaining({ payload: expect.objectContaining({ pixelWidth: 1640 }) })
    );
  });
});

describe("exportPages", () => {
  it("invokes export_pages with the render seq and page indices", () => {
    exportPages(7, [0, 1, 2]);
    expect(invoke).toHaveBeenCalledWith("export_pages", { seq: 7, pages: [0, 1, 2], pixelWidth: null });
  });

  it("forwards the target raster pixel width", () => {
    exportPages(7, [0], 1640);
    expect(invoke).toHaveBeenCalledWith("export_pages", { seq: 7, pages: [0], pixelWidth: 1640 });
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

describe("openPendingDocument", () => {
  it("invokes open_pending_document with no arguments (backend owns the path)", () => {
    openPendingDocument();
    expect(invoke).toHaveBeenCalledWith("open_pending_document");
  });
});

describe("frontendReady", () => {
  it("invokes frontend_ready so the backend emits a pending open request", () => {
    frontendReady();
    expect(invoke).toHaveBeenCalledWith("frontend_ready");
  });
});

describe("confirmClose", () => {
  it("invokes confirm_close after the dirty guard approved", () => {
    confirmClose();
    expect(invoke).toHaveBeenCalledWith("confirm_close");
  });
});

describe("recovery commands (M9 task 4)", () => {
  it("saveRecoverySnapshot sends the markdown; the backend owns the key", () => {
    saveRecoverySnapshot("# Draft");
    expect(invoke).toHaveBeenCalledWith("save_recovery_snapshot", {
      markdown: "# Draft",
    });
  });

  it("takeRecovery asks for the newest snapshot", () => {
    takeRecovery();
    expect(invoke).toHaveBeenCalledWith("take_recovery");
  });

  it("restoreRecovery and discardRecovery address one snapshot by id", () => {
    restoreRecovery("0123456789abcdef");
    expect(invoke).toHaveBeenCalledWith("restore_recovery", {
      id: "0123456789abcdef",
    });
    discardRecovery("0123456789abcdef");
    expect(invoke).toHaveBeenCalledWith("discard_recovery", {
      id: "0123456789abcdef",
    });
  });

  it("clearRecovery drops the current document's snapshot", () => {
    clearRecovery();
    expect(invoke).toHaveBeenCalledWith("clear_recovery");
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

describe("saveDocument", () => {
  it("invokes save_document with only the content (backend owns the path)", () => {
    saveDocument("# Hi");
    expect(invoke).toHaveBeenCalledWith("save_document", { content: "# Hi" });
  });
});

describe("forgetDocument", () => {
  it("invokes forget_document with no arguments (New)", () => {
    forgetDocument();
    expect(invoke).toHaveBeenCalledWith("forget_document");
  });
});

describe("exportPdf", () => {
  it("invokes export_pdf with the render payload, title and directory", () => {
    exportPdf("# Hi", OPTIONS, "My Doc", "/home/u/Documents");
    expect(invoke).toHaveBeenCalledWith("export_pdf", {
      payload: {
        markdown: "# Hi",
        options: renderOptionsPayload(OPTIONS),
        seq: 0,
        documentPath: null,
      },
      title: "My Doc",
      directory: "/home/u/Documents",
    });
  });

  it("forwards the document path for document-relative images", () => {
    exportPdf("# Hi", OPTIONS, "My Doc", "/home/u/Documents", "/home/u/docs/report.md");
    expect(invoke).toHaveBeenCalledWith("export_pdf", {
      payload: {
        markdown: "# Hi",
        options: renderOptionsPayload(OPTIONS),
        seq: 0,
        documentPath: "/home/u/docs/report.md",
      },
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
    setWindowTitle("Sample - MarkdownBláthy");
    expect(getCurrentWindow().setTitle).toHaveBeenCalledWith(
      "Sample - MarkdownBláthy"
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

describe("updateFrontMatter", () => {
  it("invokes update_front_matter with the markdown and the patch", () => {
    const patch = { title: "My Doc", autoHeadingLevel: 2 };
    updateFrontMatter("# Body", patch);
    expect(invoke).toHaveBeenCalledWith("update_front_matter", {
      markdown: "# Body",
      patch,
    });
  });
});

describe("validateTemplate", () => {
  it("invokes validate_template with the template path and the current options", () => {
    validateTemplate("/home/u/custom.typ", OPTIONS);
    expect(invoke).toHaveBeenCalledWith("validate_template", {
      path: "/home/u/custom.typ",
      options: renderOptionsPayload(OPTIONS),
    });
  });
});

describe("listTemplates", () => {
  it("invokes list_templates with no arguments", () => {
    listTemplates();
    expect(invoke).toHaveBeenCalledWith("list_templates");
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
