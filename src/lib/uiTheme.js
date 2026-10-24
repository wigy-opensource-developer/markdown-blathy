// UI appearance preference (§8.6 of SPEC.md): light / system / dark.
//
// This is NOT a document or page setting — it only skins the app chrome
// (toolbar, panes, status bar, editor) and has no effect on the rendered
// Typst document or the exported PDF. Persisted next to the render options in
// `settings.js`.

/** The toolbar dropdown options, in the requested order. */
export const UI_THEMES = [
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
];

/** Whether `value` is a valid UI theme preference. */
export function isUiTheme(value) {
  return UI_THEMES.some((theme) => theme.value === value);
}

/** Resolve a preference to the concrete Mantine color scheme.
 *
 *  `system` follows the OS via `prefers-color-scheme`; `light`/`dark` are
 *  fixed. The return value is always `"light"` or `"dark"` (what
 *  `MantineProvider`'s `forceColorScheme` expects).
 */
export function resolveColorScheme(preference, prefersDark) {
  if (preference === "dark") return "dark";
  if (preference === "light") return "light";
  return prefersDark ? "dark" : "light";
}
