// Build-output guard: no WOFF 1.0 files (and no dangling `.woff` references
// in the CSS) may land in `dist/`.
//
// Vite copies every `url()` an @font-face references; Fontsource rules list a
// `.woff` fallback after the `.woff2`. Tauri 2 webviews all support WOFF2, so
// the `woff2Only()` plugin (scripts/woff2-only.mjs) strips the fallback before
// asset resolution. This check fails the build if a font bump or a config
// change lets the ~4 MB back in.
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
const woffFiles = files.filter((path) => path.endsWith(".woff"));
const cssWithWoff = files
  .filter((path) => path.endsWith(".css"))
  .filter((path) => /\.woff(?!2)/.test(readFileSync(path, "utf8")));

if (woffFiles.length > 0 || cssWithWoff.length > 0) {
  for (const path of woffFiles) console.error(`error: WOFF 1.0 file in dist: ${path}`);
  for (const path of cssWithWoff) console.error(`error: dangling .woff reference in: ${path}`);
  console.error(
    "\nThe woff2Only() Vite plugin (scripts/woff2-only.mjs) should strip these; " +
      "check it is still registered in vite.config.js."
  );
  process.exit(1);
}

console.log("bundle check: no WOFF 1.0 files in dist");
