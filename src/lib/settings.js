// localStorage-backed settings (§8.6 of SPEC.md).
//
// v1 persists under one JSON key. The storage object is injected so the
// module stays pure and unit-testable in node (which has no
// `window.localStorage`); the app passes `window.localStorage`.

const STORAGE_KEY = "tauri-md-editor.settings";

/** The settings shape (§8.6): toolbar options plus the editor font size. */
export const DEFAULT_SETTINGS = {
  options: {
    flavor: "gfm",
    paper: "a4",
    marginPreset: "normal",
    margins: { topMm: 25, bottomMm: 25, leftMm: 20, rightMm: 20 },
    textAlign: "left",
    templatePath: null,
  },
  editorFontSize: 16,
};

/** Read settings, falling back to the defaults on corrupt/missing data (§8.6).
 *  Partial stored objects are merged onto the defaults so a schema change in a
 *  later version degrades gracefully instead of breaking the app. */
export function loadSettings(storage) {
  try {
    const raw = storage.getItem(STORAGE_KEY);
    if (!raw) return structuredClone(DEFAULT_SETTINGS);
    return mergeSettings(JSON.parse(raw));
  } catch {
    return structuredClone(DEFAULT_SETTINGS);
  }
}

/** Persist settings as JSON under the settings key (§8.6). */
export function saveSettings(settings, storage) {
  storage.setItem(STORAGE_KEY, JSON.stringify(settings));
}

// Option fields where `null` is a real value rather than Select deselect
// noise: `templatePath: null` means "the embedded default template" (clearing
// the custom template, §8.4).
const NULLABLE_OPTION_FIELDS = new Set(["templatePath"]);

/** Merge a partial patch into the toolbar options, dropping null values for
 *  fields that cannot be empty. Mantine Select fires `onChange(null)` when
 *  the already-selected option is clicked again (`allowDeselect` defaults to
 *  true); applying that blindly would corrupt the settings and the IPC
 *  payload (§13). */
export function applyOptionsPatch(options, patch) {
  const cleaned = {};
  for (const [field, value] of Object.entries(patch)) {
    if (value === null && !NULLABLE_OPTION_FIELDS.has(field)) continue;
    cleaned[field] = value;
  }
  return { ...options, ...cleaned };
}

/** Merge parsed settings onto the defaults, dropping anything malformed. */
function mergeSettings(parsed) {
  if (parsed === null || typeof parsed !== "object") {
    return structuredClone(DEFAULT_SETTINGS);
  }
  const defaults = DEFAULT_SETTINGS;
  const storedOptions = parsed.options ?? {};
  const options = { ...defaults.options, ...storedOptions };
  // The IPC payload requires a complete 4-field margins object (§6.1); a
  // partial/null stored value falls back to the default preset margins.
  if (!hasMargins(options.margins)) {
    options.margins = defaults.options.margins;
  }
  return { ...defaults, ...parsed, options };
}

/** True when `margins` is a complete 4-field millimetre object. */
function hasMargins(margins) {
  return (
    margins !== null &&
    typeof margins === "object" &&
    [margins.topMm, margins.bottomMm, margins.leftMm, margins.rightMm].every(
      (v) => typeof v === "number"
    )
  );
}
