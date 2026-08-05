// Title/directory derivation tests (§8.4 of SPEC.md).
//
// The frontmatter field parser mirrors `markdown/convert.rs`'s
// `parse_front_matter`/`yaml_field` rules (split on first ": ", trim, strip
// one pair of surrounding double quotes) so the Save/Export default file
// names stay consistent with the title the Typst template renders.
import { describe, expect, it } from "vitest";
import {
  docTitle,
  filenameStem,
  frontmatterTitle,
  parentDirectory,
  safeFileName,
  windowTitle,
} from "./title.js";

describe("frontmatterTitle", () => {
  it("reads a quoted title", () => {
    expect(frontmatterTitle('---\ntitle: "My Doc"\nauthor: "Ada"\n---\n\nBody')).toBe(
      "My Doc"
    );
  });

  it("reads an unquoted title", () => {
    expect(frontmatterTitle("---\ntitle: Plain Doc\n---\n\nBody")).toBe("Plain Doc");
  });

  it("ignores other frontmatter keys", () => {
    expect(
      frontmatterTitle('---\ndate: "2026-01-01"\ntags: [a]\ntitle: "T"\n---\n\nBody')
    ).toBe("T");
  });

  it("returns null when there is no title field", () => {
    expect(frontmatterTitle("---\nauthor: Ada\n---\n\nBody")).toBeNull();
  });

  it("returns null when the document has no frontmatter block", () => {
    expect(frontmatterTitle("# No frontmatter here")).toBeNull();
  });

  it("returns null when the opening delimiter is not on the first line", () => {
    expect(frontmatterTitle("# Title\n\n---\ntitle: Late\n---\n")).toBeNull();
  });

  it("returns null when the block is unterminated", () => {
    expect(frontmatterTitle("---\ntitle: Never closed\n\nBody")).toBeNull();
  });
});

describe("docTitle", () => {
  it("prefers the frontmatter title over the filename", () => {
    const md = "---\ntitle: Front Matter Title\n---\n\nBody";
    expect(docTitle(md, "/a/b/file.md", "Sample")).toBe("Front Matter Title");
  });

  it("falls back to the filename stem without frontmatter", () => {
    expect(docTitle("# Body", "/a/b/notes.md", "Sample")).toBe("notes");
  });

  it("falls back to the default when there is no frontmatter and no file", () => {
    expect(docTitle("# Body", null, "Sample")).toBe("Sample");
  });

  it("treats an empty frontmatter title as absent", () => {
    expect(docTitle('---\ntitle: ""\n---\n\nBody', "/a/b/notes.md", "Sample")).toBe("notes");
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
