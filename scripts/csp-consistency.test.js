// CSP ↔ CodeMirror consistency guard.
//
// Tauri rewrites the configured CSP before serving the app, injecting a
// nonce into `style-src` (and onto build-time <style> tags). Per CSP3, the
// presence of a nonce makes `'unsafe-inline'` moot — so every <style> that
// CodeMirror 6 and Mantine's CSS-in-JS inject at *runtime* (nonce-less,
// unavoidably) is silently dropped. The visible symptom is an editor whose
// text layer collapses (the cm-scroller loses `display: flex`, so gutters
// stack above the content and the window shows only line numbers).
//
// The defense is `dangerousDisableAssetCspModification: ["style-src"]`,
// which leaves the author-declared style-src list untouched. WebKitGTK
// 2.52.6 began enforcing this strictly (it silently passed before), so a
// config that regresses here breaks the editor on every modern webview.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const config = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8")
);

describe("tauri.conf.json CSP", () => {
  const csp = config.app?.security?.csp;
  const exemptions = config.app?.security?.dangerousDisableAssetCspModification;

  it("is a string with a style-src directive (or absent entirely)", () => {
    if (csp === undefined) return; // no CSP at all: nothing to keep consistent
    expect(typeof csp).toBe("string");
    expect(csp).toMatch(/style-src/);
  });

  it("exempts style-src from Tauri's nonce injection while a CSP exists", () => {
    if (csp === undefined) return;
    // Must be the *list* form naming style-src. `true` would also exempt the
    // scripts Tauri itself injects (breaking the IPC bootstrap), and the
    // default `false`/missing lets the nonce poison style-src.
    expect(Array.isArray(exemptions)).toBe(true);
    expect(exemptions).toContain("style-src");
  });

  it("keeps 'unsafe-inline' in style-src (what the exemption protects)", () => {
    if (csp === undefined) return;
    const styleSrc = csp.split(";").find((d) => d.trim().startsWith("style-src"));
    expect(styleSrc).toContain("'unsafe-inline'");
  });
});
