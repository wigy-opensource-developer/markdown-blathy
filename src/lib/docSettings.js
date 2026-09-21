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
