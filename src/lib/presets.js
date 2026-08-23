// Option tables for the toolbar selects (§8.6 of SPEC.md).
//
// `value` fields are the wire values the Rust side deserializes — the serde
// enums in `src-tauri/src/options.rs` use `rename_all = "snake_case"`, so the
// select values must be exactly `a4`/`left`/etc. Flavor is not a toolbar
// select (it is a per-document front-matter setting), so there is no table
// for it here.

export const PAPERS = [
  { value: "a4", label: "A4" },
  { value: "a5", label: "A5" },
  { value: "letter", label: "Letter" },
  { value: "legal", label: "Legal" },
];

export const MARGIN_PRESETS = [
  { value: "normal", label: "Normal (20/25mm)" },
  { value: "narrow", label: "Narrow (10/15mm)" },
  { value: "custom", label: "Custom…" },
];

export const ALIGNS = [
  { value: "left", label: "Left" },
  { value: "center", label: "Center" },
  { value: "justified", label: "Justified" },
];

// Toggleable Markdown extensions (§6.1). `key` matches the `FeatureFlags`
// field names (snake_case, same as the `ext_*` front-matter keys) that Rust
// serializes into the render result's `doc.features`.
export const FEATURES = [
  { key: "strikethrough", label: "Strikethrough ~~text~~" },
  { key: "table", label: "Tables" },
  { key: "task_list", label: "Task lists" },
  { key: "footnotes", label: "Footnotes" },
  { key: "math", label: "Math $…$" },
  { key: "highlight", label: "Highlight ==text==" },
  { key: "superscript", label: "Superscript ^text^" },
  { key: "subscript", label: "Subscript ~text~" },
  { key: "definition_list", label: "Definition lists" },
  { key: "page_break", label: "Page breaks" },
  { key: "page_toc", label: "Table of Contents [[_TOC_]]" },
];

const ALL_OFF = Object.fromEntries(FEATURES.map((f) => [f.key, false]));
const ALL_ON = Object.fromEntries(FEATURES.map((f) => [f.key, true]));

// Extension presets matching the Rust `Flavor::preset` table (§6.1):
// CommonMark enables nothing, GitHub the GFM set, Custom everything.
export const FEATURE_PRESETS = {
  commonmark: { ...ALL_OFF },
  github: { ...ALL_OFF, strikethrough: true, table: true, task_list: true },
  custom: { ...ALL_ON },
};

// Preset → page margins in millimetres. The template's `x`/`y` shorthand uses
// the smaller of left/right and top/bottom respectively (§7.4), so the preset
// values here keep `x < y` to match the labels.
const MARGIN_PRESET_MM = {
  normal: { topMm: 25, bottomMm: 25, leftMm: 20, rightMm: 20 },
  narrow: { topMm: 15, bottomMm: 15, leftMm: 10, rightMm: 10 },
  custom: null,
};

/** Margins (mm) for a margin preset, or `null` for `custom` (user-edited
 *  margins arrive in a later milestone). */
export function marginsForPreset(value) {
  return MARGIN_PRESET_MM[value] ?? null;
}

/** Build the `options` object sent over IPC — the shape `RenderOptions`
 *  deserializes (camelCase), without the frontend-only `marginPreset` key. */
export function renderOptionsPayload(options) {
  return {
    flavor: options.flavor,
    paper: options.paper,
    margins: options.margins,
    textAlign: options.textAlign,
    templatePath: options.templatePath ?? null,
  };
}
