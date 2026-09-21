import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { woff2Only } from "./scripts/woff2-only.mjs";

// Tauri expects a fixed dev port and serves the built frontend from `dist`.
export default defineConfig({
  // `woff2Only` drops the WOFF-1.0 fallbacks Fontsource lists after every
  // WOFF2 source: no Tauri 2 webview can reach them, and they cost ~4 MB.
  // scripts/check-bundle.mjs keeps them out of `dist` in CI.
  plugins: [react(), woff2Only()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
