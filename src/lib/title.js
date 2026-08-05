// Title & directory derivation for the current document (§8.4 of SPEC.md).
//
// The frontmatter field parser mirrors `markdown/convert.rs`:
// `parse_front_matter`/`yaml_field`/`clean_yaml_value` — split on the first
// ": ", trim, strip one pair of surrounding double quotes — so the default
// Save/Export file names agree with the title the Typst template renders.

/** Extract the `title` field from the leading `---`-delimited frontmatter
 *  block, or null when absent. Mirrors the Rust parser's rules. */
export function frontmatterTitle(markdown) {
  const block = frontmatterBlock(markdown);
  if (block === null) return null;
  for (const line of block.split("\n")) {
    const title = yamlField(line, "title");
    if (title !== null) return title;
  }
  return null;
}

/** The leading `---`-delimited block (delimiters stripped), or null. */
function frontmatterBlock(markdown) {
  const lines = String(markdown ?? "").split("\n");
  if (lines[0] !== "---") return null;
  const close = lines.indexOf("---", 1);
  if (close === -1) return null;
  return lines.slice(1, close).join("\n");
}

/** Read one `key: value` field from a YAML line (mirrors Rust `yaml_field`). */
function yamlField(line, key) {
  const idx = line.indexOf(": ");
  if (idx === -1) return null;
  if (line.slice(0, idx).trim() !== key) return null;
  return cleanYamlValue(line.slice(idx + 2));
}

/** Trim whitespace and strip a pair of surrounding double quotes. */
function cleanYamlValue(value) {
  const trimmed = value.trim();
  if (trimmed.length >= 2 && trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return trimmed.slice(1, -1).trim();
  }
  return trimmed;
}

/** The title of the current document: frontmatter `title` wins, then the
 *  file stem, then `fallback`. Empty values fall through (§8.4). */
export function docTitle(markdown, filePath, fallback = "Sample") {
  return frontmatterTitle(markdown) || filenameStem(filePath) || fallback;
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
