import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { woff2Only } from "./scripts/woff2-only.mjs";

// Tauri expects a fixed dev port and serves the built frontend from `dist`.
export default defineConfig({
  // `woff2Only` drops the WOFF-1.0 fallbacks Fontsource lists after every
  // WOFF2 source: no Tauri 2 webview can reach them, and they cost ~4 MB.
  // scripts/check-bundle.mjs keeps them out of `dist` in CI.
  plugins: [react(), woff2Only()],
  resolve: {
    // The editor highlights Markdown only: `markdown()` gets no
    // `codeLanguages`, so `@codemirror/lang-markdown`'s transitive js/css
    // grammars (~110 kB minified, dominated by `@lezer/javascript`) would
    // only ever run on `<script>`/`<style>` bodies inside raw HTML blocks.
    // Swap in inert parsers instead — inline HTML and `<div>` tag completion
    // keep working (they live in `@codemirror/lang-html` + `@lezer/html`),
    // and the app ships no JavaScript/CSS parser at all. The aliases must
    // apply to tests too (scripts/stubs/lang-html.test.js), so they live
    // here rather than in a plugin; keep them in sync with
    // scripts/check-bundle.mjs, which fails the build if the grammars
    // reappear in `dist`.
    alias: {
      "@codemirror/lang-javascript": fileURLToPath(new URL("./scripts/stubs/codemirror-lang-javascript.mjs", import.meta.url)),
      "@codemirror/lang-css": fileURLToPath(new URL("./scripts/stubs/codemirror-lang-css.mjs", import.meta.url)),
    },
  },
  test: {
    server: {
      deps: {
        // Vitest externalizes node_modules by default, and externalized
        // imports bypass `resolve.alias`; inline the two packages whose
        // import chains the aliases must rewrite.
        inline: [/@codemirror\/lang-(markdown|html)/],
      },
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
