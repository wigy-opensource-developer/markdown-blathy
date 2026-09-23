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

import { createTheme, MantineProvider } from "@mantine/core";
import App from "./App.jsx";
import "./styles.css";

const theme = createTheme({
  fontFamily: "Archivo Narrow Variable, system-ui, sans-serif",
  headings: { fontFamily: "Archivo Narrow Variable, system-ui, sans-serif" },
  // Archivo Narrow is a condensed face, so it reads smaller than its px size
  // suggests; a noticeably larger scale than the Mantine defaults keeps the
  // toolbar/labels legible.
  fontSizes: {
    xs: "15px",
    sm: "17px",
    md: "19px",
    lg: "21px",
    xl: "23px",
  },
});

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="light">
      <App />
    </MantineProvider>
  </React.StrictMode>
);
