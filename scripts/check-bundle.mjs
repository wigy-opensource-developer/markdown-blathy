// Build-output guard: three properties of `dist/` that keep the bundle lean.
//
// 1. No WOFF 1.0 files (and no dangling `.woff` references in the CSS).
//    Vite copies every `url()` an @font-face references; Fontsource rules list
//    a `.woff` fallback after the `.woff2`. Tauri 2 webviews all support
//    WOFF2, so the `woff2Only()` plugin (scripts/woff2-only.mjs) strips the
//    fallback before asset resolution. This check fails the build if a font
//    bump or a config change lets the ~4 MB back in.
//
// 2. No unused static Iosevka weights. `@fontsource/iosevka` ships Iosevka
//    *Complete* (31 stylistic sets, huge glyph coverage): ~1 MB per weight,
//    even for the `latin` subset. The editor pins weight 500
//    (`src/styles.css`, `.cm-scroller`), so every other weight is dead bytes
//    the webview never fetches.
//
// 3. No JavaScript/CSS grammars in the editor chunk. This editor highlights
//    Markdown only: `markdown()` is configured without `codeLanguages`, so the
//    only live use of `@codemirror/lang-javascript`/`lang-css` is inside raw
//    `<script>`/`<style>` content of inline HTML blocks. The vite config
//    aliases both packages to no-nested-grammar stubs
//    (`scripts/stubs/`), which keeps inline-HTML highlighting and `<div>`
//    tag completion while dropping ~110 kB of minified grammar — and, per
//    `cargo deny`'s reasoning for JS parsers, shrinks the frontend's attack
//    surface to grammars this app never parses. These marker strings appear
//    only in `@lezer/javascript`/`@lezer/css` node tables; if one shows up in
//    `dist/`, an alias was removed or a dependency started importing the
//    grammars directly.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const dist = resolve(dirname(fileURLToPath(import.meta.url)), "..", "dist");

/** Every file in `dir`, recursively. */
function filesIn(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? filesIn(path) : [path];
  });
}

if (!statSync(dist, { throwIfNoEntry: false })?.isDirectory()) {
  console.error("error: dist/ not found — run `npm run build` first");
  process.exit(1);
}

const files = filesIn(dist);
const problems = [];

// 1. WOFF 1.0 must not survive into the bundle.
const woffFiles = files.filter((path) => path.endsWith(".woff"));
const cssWithWoff = files
  .filter((path) => path.endsWith(".css"))
  .filter((path) => /\.woff(?!2)/.test(readFileSync(path, "utf8")));
for (const path of woffFiles) problems.push(`WOFF 1.0 file in dist: ${path}`);
for (const path of cssWithWoff) problems.push(`dangling .woff reference in: ${path}`);
if (woffFiles.length > 0 || cssWithWoff.length > 0) {
  problems.push(
    "The woff2Only() Vite plugin (scripts/woff2-only.mjs) should strip these; " +
      "check it is still registered in vite.config.js."
  );
}

// 2. Only the Iosevka weights the app actually styles with may be bundled.
const ALLOWED_FONT_WEIGHTS = { Iosevka: ["500"] };
for (const path of files.filter((p) => p.endsWith(".css"))) {
  for (const face of readFileSync(path, "utf8").matchAll(/@font-face\s*{[^}]*}/gi)) {
    const family = face[0].match(/font-family:\s*(?:'|")?([^;'",}]+)/i)?.[1]?.trim();
    const weight = face[0].match(/font-weight:\s*([^;}]+)/i)?.[1]?.trim();
    if (family && family in ALLOWED_FONT_WEIGHTS && !ALLOWED_FONT_WEIGHTS[family].includes(weight)) {
      problems.push(
        `unused font weight bundled: ${family} ${weight} (${path}) — each static ` +
          `Iosevka weight costs ~1 MB; only bundle weights CSS selects ` +
          `(see ALLOWED_FONT_WEIGHTS in this script)`
      );
    }
  }
}

// 3. The JS/CSS grammars the stubs remove must stay out of the JS.
const FORBIDDEN_JS_MARKERS = {
  ArrowFunction: "@lezer/javascript",
  ColorLiteral: "@lezer/css",
};
for (const path of files.filter((p) => p.endsWith(".js"))) {
  const code = readFileSync(path, "utf8");
  for (const [marker, pkg] of Object.entries(FORBIDDEN_JS_MARKERS)) {
    if (code.includes(marker)) {
      problems.push(
        `grammar marker "${marker}" (${pkg}) in ${path} — the editor is ` +
          `markdown-only; the scripts/stubs/ aliases in vite.config.js should ` +
          `keep ${pkg} out of the bundle`
      );
    }
  }
}

if (problems.length > 0) {
  for (const problem of problems) console.error(`error: ${problem}`);
  process.exit(1);
}

console.log("bundle check: no WOFF 1.0 files, only bundled font weights are used, no js/css grammars");
