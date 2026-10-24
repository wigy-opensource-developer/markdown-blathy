import React from "react";
import ReactDOM from "react-dom/client";

// Self-hosted fonts (bundled by Vite; no external requests).
// Only the Iosevka weight the editor actually styles with (500, pinned on
// `.cm-scroller` in styles.css) — each static Iosevka weight is ~1 MB of
// woff2; scripts/check-bundle.mjs fails the build if another one shows up.
import "@fontsource/iosevka/500.css";
// Archivo Narrow Variable: single file, wght axis 400-700 (real 400/500/600/700
// instances). The axis CSS bundles the latin + latin-ext subsets via
// unicode-range, so Hungarian etc. diacritics (ő, ű) are covered too.
import "@fontsource-variable/archivo-narrow/wght.css";

// Mantine styles (must be imported once, before component styles).
import "@mantine/core/styles.css";

import App from "./App.jsx";
import { loadSettings } from "./lib/settings.js";
import { resolveColorScheme } from "./lib/uiTheme.js";
import "./styles.css";

// Apply the persisted color scheme before the first paint so a dark preference
// never flashes light. App re-asserts (and keeps in sync for "system") via
// MantineProvider's forceColorScheme; this pre-paint attribute only covers the
// first frame and the boot note.
const storedTheme = loadSettings(window.localStorage).theme;
const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
document.documentElement.setAttribute(
  "data-mantine-color-scheme",
  resolveColorScheme(storedTheme, prefersDark)
);

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
