// Title/directory derivation tests (§8.4 of SPEC.md).
//
// Front matter is parsed by Rust (`markdown/frontmatter.rs`) and returned
// with the render result; `docTitle` only combines that parsed title with
// the file name and a fallback.
import { describe, expect, it } from "vitest";
import {
  docTitle,
  filenameStem,
  parentDirectory,
  safeFileName,
  windowTitle,
} from "./title.js";

describe("docTitle", () => {
  it("prefers the parsed frontmatter title over the filename", () => {
    expect(docTitle("Front Matter Title", "/a/b/file.md", "Sample")).toBe(
      "Front Matter Title"
    );
  });

  it("falls back to the filename stem without a frontmatter title", () => {
    expect(docTitle(null, "/a/b/notes.md", "Sample")).toBe("notes");
  });

  it("falls back to the default when there is no title and no file", () => {
    expect(docTitle(null, null, "Sample")).toBe("Sample");
  });

  it("treats an empty parsed title as absent", () => {
    expect(docTitle("", "/a/b/notes.md", "Sample")).toBe("notes");
  });
});

describe("filenameStem", () => {
  it("strips the extension", () => {
    expect(filenameStem("/a/b/notes.md")).toBe("notes");
  });

  it("handles a bare filename", () => {
    expect(filenameStem("document.pdf")).toBe("document");
  });

  it("keeps names without an extension", () => {
    expect(filenameStem("/a/b/notes")).toBe("notes");
  });

  it("keeps leading-dot names as-is", () => {
    expect(filenameStem(".hidden")).toBe(".hidden");
  });

  it("returns an empty string for an empty path", () => {
    expect(filenameStem(null)).toBe("");
  });
});

describe("parentDirectory", () => {
  it("returns the parent of a nested path", () => {
    expect(parentDirectory("/a/b/notes.md")).toBe("/a/b");
  });

  it("returns the root for a top-level file", () => {
    expect(parentDirectory("/notes.md")).toBe("/");
  });

  it("returns an empty string when there is no directory", () => {
    expect(parentDirectory("notes.md")).toBe("");
  });

  it("returns the root for the root itself", () => {
    expect(parentDirectory("/")).toBe("/");
  });
});

describe("safeFileName", () => {
  it("strips path separators and reserved characters", () => {
    expect(safeFileName('a/b\\c:d*e?f"g<h>i|j')).toBe("a b c d e f g h i j");
  });

  it("collapses and trims whitespace", () => {
    expect(safeFileName("  My   Doc  ")).toBe("My Doc");
  });

  it("falls back to 'document' when nothing usable remains", () => {
    expect(safeFileName("///")).toBe("document");
    expect(safeFileName("")).toBe("document");
  });
});

describe("windowTitle", () => {
  it("suffixes the document title with the app name", () => {
    expect(windowTitle("Sample")).toBe("Sample - Markdown to PDF");
  });
});
