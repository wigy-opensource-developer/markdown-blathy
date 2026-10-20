// Document settings form ↔ front-matter patch mapping (§6.2).
//
// The Document popover holds a small form while it is open and turns it into
// the patch `updateFrontMatter` applies. `hyphenate` is tri-state over IPC:
// an absent key means "keep", JSON `null` means "clear" (back to auto) and a
// boolean sets it — so `false` must travel as `false`, never as "missing".
// `lang` clears with an empty string, like title/author.

/** The doc-meta value (`null` = auto) as the hyphenation Select value. */
export const hyphenateFormValue = (hyphenate) =>
  hyphenate == null ? "auto" : String(hyphenate);

/** The hyphenation Select value as its patch value (`null` clears the key). */
export const hyphenatePatchValue = (value) =>
  value === "auto" ? null : value === "true";

/** Compare two feature maps by value, ignoring key order. */
export const featuresEqual = (a, b) => {
  const left = a ?? {};
  const right = b ?? {};
  const keys = Object.keys(left);
  return (
    keys.length === Object.keys(right).length &&
    keys.every((key) => left[key] === right[key])
  );
};

/**
 * Diff the open form against the snapshot taken when it opened and return the
 * `updateFrontMatter` patch: only changed fields, so an untouched setting is
 * never rewritten. `form`/`initial` are the shapes `DocumentSettings` holds
 * (all fields present; `hyphenate` is a Select string).
 */
export function buildDocSettingsPatch(form, initial) {
  const patch = {};
  if (form.title !== initial.title) patch.title = form.title; // "" clears
  if (form.author !== initial.author) patch.author = form.author;
  if (form.autoHeadingLevel !== initial.autoHeadingLevel) {
    patch.autoHeadingLevel = form.autoHeadingLevel;
  }
  if (form.lang !== initial.lang) patch.lang = form.lang; // "" clears
  if (form.hyphenate !== initial.hyphenate) {
    patch.hyphenate = hyphenatePatchValue(form.hyphenate);
  }
  if (!featuresEqual(form.features, initial.features)) {
    patch.features = form.features;
  }
  return patch;
}

/**
 * Merge an already-applied front-matter patch into the last render's docMeta.
 * The patch is canonical (Rust wrote it), so the popover can show the new
 * values immediately instead of waiting for the follow-up render — which may
 * be deferred while the popover is open, or fail on unrelated document
 * content and leave the old metadata in place. Absent keys keep the current
 * value; empty title/author/lang clear it (the front-matter key was removed).
 */
export function applyDocSettingsPatch(docMeta, patch) {
  if (!docMeta) return docMeta;
  const next = { ...docMeta };
  if (patch.title !== undefined) next.title = patch.title || null;
  if (patch.author !== undefined) next.author = patch.author || null;
  if (patch.autoHeadingLevel !== undefined) {
    next.autoHeadingLevel = patch.autoHeadingLevel;
  }
  if (patch.lang !== undefined) next.lang = patch.lang || null;
  if (patch.hyphenate !== undefined) next.hyphenate = patch.hyphenate;
  if (patch.features !== undefined) next.features = { ...patch.features };
  return next;
}
