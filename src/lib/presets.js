// Option tables for the toolbar selects (§8.6 of SPEC.md).
//
// `value` fields are the wire values the Rust side deserializes — the serde
// enums in `src-tauri/src/options.rs` use `rename_all = "snake_case"`, so the
// select values must be exactly `commonmark`/`github`/`custom`, etc.

export const FLAVORS = [
  { value: "commonmark", label: "CommonMark" },
  { value: "github", label: "GitHub" },
  { value: "custom", label: "Custom" },
];

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
