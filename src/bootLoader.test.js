// Guards the boot loader markup/style in the repo-root `index.html` — the
// one piece of first-paint UI that exists outside React (it renders while
// the JS bundle is still being fetched/evaluated, and React clears `#root`
// on mount). Startup regressions here are invisible to every other test.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

describe("index.html boot loader", () => {
  it("lives inside #root so React clears it on mount without extra code", () => {
    const root = html.match(/<div id="root">([\s\S]*?)<\/div>\s*<script/);
    expect(root, "loader must sit inside #root, before the module script").not.toBeNull();
    expect(root[1]).toContain("tilted-note");
  });

  it("carries the start-up message", () => {
    expect(html).toContain("MarkdownBláthy is starting");
  });

  it("shares the tilted-note rotation with the app stylesheet (the quirk)", () => {
    const inHtml = html.match(/transform:\s*rotate\((-?[\d.]+)deg\)/);
    const inCss = css.match(/\.tilted-note > span\s*{[^}]*rotate\((-?[\d.]+)deg\)/);
    expect(inHtml, "index.html note needs a rotation").not.toBeNull();
    expect(inCss, "styles.css note needs a rotation").not.toBeNull();
    expect(inHtml[1]).toBe(inCss[1]);
  });

  it("promises a white note that survives the icon-blue window background", () => {
    // The window/webview background is the icon blue (tauri.conf.json
    // backgroundColor); the note must read on it — and on the dark preview
    // pane it later shares the style with.
    expect(html.match(/\.tilted-note\s*{[^}]*color:\s*#fff/)).not.toBeNull();
  });

  it("keeps the page body the same blue as the window backgroundColor", () => {
    // WebKit paints the document background over the window color once the
    // page commits; a mismatch is the grey flash between the blue window
    // and the app's first frame. Both values come from one source: the
    // tauri config.
    const config = JSON.parse(
      readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8")
    );
    const [r, g, b] = config.app.windows[0].backgroundColor;
    const expected = `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
    const body = html.match(/html,\s*body\s*{[^}]*background:\s*(#[0-9a-f]{6})/);
    expect(body, "html/body need the boot background color").not.toBeNull();
    expect(body[1]).toBe(expected);
  });
});
