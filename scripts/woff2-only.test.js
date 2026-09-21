// WOFF-1.0 fallback stripping for bundled @font-face rules.
//
// Fontsource ships each static face as `woff2, woff`; Vite copies both into
// dist. Every webview Tauri 2 supports (WebView2/Chromium, WKWebView/Safari
// 12+, WebKitGTK 2.38+) understands WOFF2, so the `woff` alternative is dead
// weight — the browser picks the first supported `src` and never fetches it.
// The Vite plugin drops that alternative before asset resolution, so only the
// woff2 file lands in dist.
import { describe, expect, it } from "vitest";
import { stripWoffFallbacks } from "./woff2-only.mjs";

const IOSEVKA_400 = `@font-face {
  font-family: 'Iosevka';
  font-style: normal;
  font-display: swap;
  font-weight: 400;
  src: url(./files/iosevka-latin-400-normal.woff2) format('woff2'), url(./files/iosevka-latin-400-normal.woff) format('woff');
}`;

describe("stripWoffFallbacks", () => {
  it("removes the trailing woff fallback and keeps the woff2 source", () => {
    const out = stripWoffFallbacks(IOSEVKA_400);
    expect(out).toContain(
      "src: url(./files/iosevka-latin-400-normal.woff2) format('woff2');"
    );
    expect(out).not.toMatch(/\.woff\)/);
  });

  it("strips a woff source that comes first, without leaving a stray comma", () => {
    const css = `src: url(./x.woff) format('woff'), url(./x.woff2) format('woff2');`;
    expect(stripWoffFallbacks(css)).toBe(
      "src: url(./x.woff2) format('woff2');"
    );
  });

  it("handles double quotes and extra whitespace", () => {
    const css = `src: url( ./x.woff2 ) format( "woff2" ) , url( ./x.woff ) format( "woff" ) ;`;
    expect(stripWoffFallbacks(css)).not.toMatch(/\.woff\s*\)/);
    expect(stripWoffFallbacks(css)).toContain("./x.woff2");
  });

  it("leaves woff2-only rules byte-for-byte untouched", () => {
    const css = `@font-face { src: url(./a.woff2) format('woff2'); }`;
    expect(stripWoffFallbacks(css)).toBe(css);
  });

  it("strips every face in a stylesheet", () => {
    const css = `${IOSEVKA_400}\n${IOSEVKA_400.replace("400", "500")}`;
    const out = stripWoffFallbacks(css);
    expect(out.match(/\.woff2/g)).toHaveLength(2);
    expect(out).not.toMatch(/\.woff\)/);
  });
});
