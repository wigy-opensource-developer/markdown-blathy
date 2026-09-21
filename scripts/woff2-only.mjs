// Vite plugin: drop WOFF-1.0 fallbacks from bundled @font-face rules.
//
// Fontsource ships each static face as `woff2, woff`; Vite copies both into
// dist. Every webview Tauri 2 supports (WebView2/Chromium, WKWebView/Safari
// 12+, WebKitGTK 2.38+) understands WOFF2, so the `woff` alternative is dead
// weight — the browser picks the first supported `src` and never fetches it.
// Removing it before Vite resolves the urls keeps ~4 MB of never-fetched font
// files out of the bundle (the AppImage cannot compress them away: WOFF is
// already zlib-compressed).
//
// The alternative format selector (`format('woff')`) is the 2011 fallback;
// WOFF2 is a superset feature-wise, so dropping it changes no rendering.

/**
 * Remove every `src` alternative whose url points at a `.woff` (not `.woff2`)
 * file. Pure and order-agnostic: the surviving alternatives keep their
 * relative order and the declaration stays valid.
 */
export function stripWoffFallbacks(css) {
  return css.replace(/src:([^;}]*)/gi, (declaration, value) => {
    const kept = value
      .split(",")
      .filter((alternative) => !/\.woff(?!2)\b/i.test(alternative))
      .join(",");
    return kept === value ? declaration : `src:${kept}`;
  });
}

/** The plugin itself, wired into `vite.config.js`. */
export function woff2Only() {
  return {
    name: "woff2-only",
    // Runs before Vite's CSS plugin, while the urls are still the raw
    // `./files/…` specifiers, so the stripped `.woff` urls are never resolved
    // or emitted as assets.
    enforce: "pre",
    transform(code, id) {
      if (!/\.css(?:\?|$)/.test(id) || !code.includes(".woff")) return null;
      const stripped = stripWoffFallbacks(code);
      return stripped === code ? null : stripped;
    },
  };
}
