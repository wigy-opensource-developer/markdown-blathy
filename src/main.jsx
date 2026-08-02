import React from "react";
import ReactDOM from "react-dom/client";

// Self-hosted fonts (bundled by Vite; no external requests).
import "@fontsource/iosevka/400.css";
import "@fontsource/iosevka/500.css";
import "@fontsource/pt-sans-narrow/400.css";
import "@fontsource/pt-sans-narrow/700.css";

// Mantine styles (must be imported once, before component styles).
import "@mantine/core/styles.css";

import { createTheme, MantineProvider } from "@mantine/core";
import App from "./App.jsx";
import "./styles.css";

const theme = createTheme({
  fontFamily: "PT Sans Narrow, system-ui, sans-serif",
  headings: { fontFamily: "PT Sans Narrow, system-ui, sans-serif" },
  // PT Sans Narrow is a condensed face; a slightly larger scale reads better.
  fontSizes: {
    xs: "13px",
    sm: "15px",
    md: "17px",
    lg: "19px",
    xl: "21px",
  },
});

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="light">
      <App />
    </MantineProvider>
  </React.StrictMode>
);
