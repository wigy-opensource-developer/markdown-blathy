// Title & directory derivation for the current document (§8.4 of SPEC.md).
//
// Front matter is parsed by Rust (`markdown/frontmatter.rs`) and returned
// with the render result; this module only combines that parsed title with
// the file name and a fallback to produce the display title, plus the
// file-name/directory helpers. Keeping the title derivation here means the
// default Save/Export file names agree with the title the Typst template
// renders.

/** The title of the current document: the Rust-parsed frontmatter `title`
 *  wins, then the file stem, then `fallback`. Empty values fall through
 *  (§8.4). */
export function docTitle(parsedTitle, filePath, fallback = "Sample") {
  return parsedTitle || filenameStem(filePath) || fallback;
}

/** The file stem of a path: "/a/b/notes.md" → "notes" (§8.4). */
export function filenameStem(path) {
  const base = String(path ?? "").split("/").pop() ?? "";
  const dot = base.lastIndexOf(".");
  return dot > 0 ? base.slice(0, dot) : base;
}

/** The parent directory of a path: "/a/b/notes.md" → "/a/b"; "" when none. */
export function parentDirectory(path) {
  const p = String(path ?? "");
  const idx = p.lastIndexOf("/");
  if (idx === -1) return "";
  return idx === 0 ? "/" : p.slice(0, idx);
}

/** Sanitize a title into something safe to use as a file name: strips path
 *  separators and reserved characters (both major OSes), collapses
 *  whitespace, and falls back to "document" when nothing usable remains.
 *  Titles come from frontmatter or file names, so this keeps a crafted title
 *  like "a/b" from escaping into a directory. */
export function safeFileName(title) {
  const cleaned = String(title ?? "")
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return cleaned === "" ? "document" : cleaned;
}

/** The OS window title for the current document: "<title> - Markdown to PDF".
 *  The app-name suffix keeps multiple instances distinguishable in the
 *  window manager (§8.4). */
export function windowTitle(title) {
  return `${title} - Markdown to PDF`;
}
