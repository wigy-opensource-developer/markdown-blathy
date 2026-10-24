// The Mantine theme shared by the app and its tests.
//
// Archivo Narrow is a condensed face, so it reads smaller than its px size
// suggests; a noticeably larger scale than the Mantine defaults keeps the
// toolbar/labels legible (and helps WCAG AA text sizing at 100% zoom).
import { createTheme } from "@mantine/core";

export const mantineTheme = createTheme({
  fontFamily: "Archivo Narrow Variable, system-ui, sans-serif",
  headings: { fontFamily: "Archivo Narrow Variable, system-ui, sans-serif" },
  fontSizes: {
    xs: "15px",
    sm: "17px",
    md: "19px",
    lg: "21px",
    xl: "23px",
  },
});
