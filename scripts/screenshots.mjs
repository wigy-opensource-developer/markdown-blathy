// Regenerate docs/screenshots/ from the real app, headlessly.
//
// Orchestrated by scripts/screenshots.sh (Xvfb + openbox + picom + tauri-driver).
// Drives the app through WebDriver to each state, then captures the decorated
// window (title bar + compositor shadow) with maim on a white background.
//
// Usage: node scripts/screenshots.mjs <path-to-app-binary>
import { execSync } from "node:child_process";
import { mkdirSync } from "node:fs";

const BASE = "http://127.0.0.1:4444";
const BINARY = process.argv[2];
const OUT = "docs/screenshots";
const WIN_W = 1000;
const WIN_H = 720;

if (!BINARY) {
  console.error("usage: node scripts/screenshots.mjs <app-binary>");
  process.exit(1);
}

async function wd(method, path, body, timeoutMs = 60000) {
  const res = await fetch(BASE + path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(timeoutMs),
  });
  const json = await res.json().catch(() => ({}));
  if (!res.ok) throw new Error(`${method} ${path} -> ${res.status}: ${JSON.stringify(json).slice(0, 300)}`);
  return json.value !== undefined ? json.value : json;
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const exec = (sid, script, args = []) => wd("POST", `/session/${sid}/execute/sync`, { script, args });
async function waitFor(sid, script, tries = 80) {
  for (let i = 0; i < tries; i++) {
    if (await exec(sid, script)) return;
    await sleep(400);
  }
  throw new Error("timeout waiting for: " + script);
}
const click = (sid, selector) =>
  exec(sid, `document.querySelector(arguments[0]).click(); return true;`, [selector]);

// Reload the webview to a clean state (no popovers); the theme persists in
// localStorage and the WebDriver session survives the navigation. More
// reliable than synthesizing popover-dismiss events.
async function reload(sid) {
  try {
    await exec(sid, "location.reload(); return true;");
  } catch {
    /* the navigation tears down the execute context */
  }
  await sleep(600);
  await waitFor(sid, "return document.querySelectorAll('#preview .page-img').length > 0;");
  await exec(sid, "return document.fonts.ready.then(() => true);");
  await sleep(900);
}

// Pick a theme menu entry by label (Light / System / Dark).
async function selectTheme(sid, label) {
  await click(sid, '[data-command="theme.menu"]');
  await sleep(450);
  await exec(
    sid,
    `const item = [...document.querySelectorAll('[role="menuitem"]')].find(e => e.textContent.trim() === arguments[0]);
     if (item) item.click();
     return !!item;`,
    [label]
  );
  await sleep(800);
}

// Capture the decorated app window (client + WM title bar), then composite a
// soft shadow on a solid background with ImageMagick. A full-screen grab on
// the black root is trimmed to the window's bounding box (title bar included),
// which avoids fragile frame-geometry math; the composite supplies the
// background and shadow deterministically. Light shots use white; the dark
// theme shot uses a dark background to match.
function capture(name, bg = "#ffffff") {
  const tmp = `/tmp/mb-shot-${name}.png`;
  execSync(`maim --hidecursor ${tmp}`);
  execSync(`convert ${tmp} -fuzz 5% -trim +repage ${tmp}.trim.png`);
  execSync(
    `convert ${tmp}.trim.png \\( +clone -background "#000000" -shadow 55x12+0+6 \\) +swap ` +
      `-background "${bg}" -layers merge +repage -bordercolor "${bg}" -border 28 ${OUT}/${name}.png`
  );
  console.log(`captured ${name}.png`);
}

mkdirSync(OUT, { recursive: true });
const { sessionId: sid } = await wd("POST", "/session", {
  capabilities: { alwaysMatch: { "tauri:options": { application: BINARY } } },
});

try {
  // Size the window and wait for the sample document to render.
  await wd("POST", `/session/${sid}/window/rect`, { width: WIN_W, height: WIN_H });
  await waitFor(sid, "return document.querySelectorAll('#preview .page-img').length > 0;");
  await exec(sid, "return document.fonts.ready.then(() => true);");
  await sleep(1200); // let rasters decode + layout settle

  // 1. Main screen, light theme (forced — settings persist across runs).
  await selectTheme(sid, "Light");
  await exec(sid, "document.activeElement?.blur?.(); return true;");
  await sleep(300);
  capture("main-screen");

  // 2. Document settings popover.
  await click(sid, '[data-command="document.settings"]');
  await sleep(700);
  capture("document-options");
  await reload(sid);

  // 3. Page settings popover.
  await click(sid, '[data-command="page.settings"]');
  await sleep(700);
  capture("page-settings");
  await reload(sid);

  // 4. Dark UI theme AND dark document template, on a dark background. Set
  // both via persisted settings and reload so the preview renders the bundled
  // dark template — a white page would glare against the dark UI.
  try {
    await exec(sid, `
      const s = JSON.parse(localStorage.getItem('markdown-blathy.settings') || '{}');
      s.options = { ...(s.options || {}), templateName: 'dark', templatePath: null };
      s.theme = 'dark';
      localStorage.setItem('markdown-blathy.settings', JSON.stringify(s));
      location.reload();
      return true;
    `);
  } catch { /* the navigation tears down the execute context */ }
  await sleep(600);
  await waitFor(sid, "return document.querySelectorAll('#preview .page-img').length > 0;");
  await exec(sid, "return document.fonts.ready.then(() => true);");
  await sleep(1200);
  await exec(sid, "document.activeElement?.blur?.(); return true;");
  await sleep(300);
  capture("main-screen-dark", "#1a1b1e");

  console.log("all screenshots captured");
} finally {
  await wd("DELETE", `/session/${sid}`).catch(() => {});
}
