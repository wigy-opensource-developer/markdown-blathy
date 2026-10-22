#!/usr/bin/env node
/**
 * License gate for npm dependencies.
 *
 * Verifies that every production dependency's SPDX license expression only
 * uses licenses compatible with this GPL-3.0-or-later project (permissive
 * licenses, or copyleft that permits combination with GPLv3).
 *
 * `npm ls --parseable --all --omit=dev` lists the resolved directory of every
 * production package; each package.json is read to obtain its `license`
 * field. The root project (our own code) is skipped — its license is
 * declared in package.json and not verified against itself.
 *
 * Run with: npm run check:licenses
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT_NAME = "markdown-blathy";

// Licenses compatible with GPL-3.0-or-later. Keep this in sync with
// src-tauri/deny.toml (same policy, different tool per ecosystem).
const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "0BSD",
  "BSL-1.0",
  "Zlib",
  "Unicode-3.0",
  "CC0-1.0",
  "Unlicense",
  "OFL-1.1",
  "MPL-2.0",
  "GPL-3.0",
  "GPL-3.0-or-later",
  "LGPL-2.1-or-later",
  "LGPL-3.0-or-later",
]);

/**
 * Split an SPDX expression into its atomic license ids, stripping
 * parentheses, AND/OR operators and anything else non-identifier.
 * Conservative policy: every atomic id must be in ALLOWED, so an OR
 * dual-license passes only when every alternative is allowed.
 */
function atomicIds(expression) {
  return String(expression ?? "")
    .replace(/[()]/g, " ")
    .split(/\s+(?:OR|AND)\s+/i)
    .flatMap((part) => part.split(/\s+/))
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

let stdout;
try {
  stdout = execFileSync(
    "npm",
    ["ls", "--parseable", "--all", "--omit=dev"],
    { encoding: "utf8" }
  );
} catch (err) {
  // `npm ls` exits non-zero for missing/extraneous packages, but still
  // prints the full path list on stdout; let the gate report the licenses.
  stdout = err.stdout;
}

const packages = [];
for (const pkgDir of stdout.split("\n").map((s) => s.trim()).filter(Boolean)) {
  let meta;
  try {
    meta = JSON.parse(readFileSync(join(pkgDir, "package.json"), "utf8"));
  } catch {
    continue; // no package.json at this path; nothing to verify
  }
  if (meta.name === ROOT_NAME) continue; // our own project
  packages.push({
    name: meta.name,
    version: meta.version,
    license: meta.license,
    dir: pkgDir,
  });
}

const violations = packages.filter((pkg) =>
  !atomicIds(pkg.license).every((id) => ALLOWED.has(id))
);

if (violations.length > 0) {
  console.error(`License gate failed: ${violations.length} package(s):`);
  for (const v of violations) {
    console.error(`  ${v.name}@${v.version}  license: ${v.license ?? "MISSING"}`);
  }
  console.error(
    "If the license is compatible with GPL-3.0-or-later, add it to ALLOWED in scripts/check-licenses.mjs (and to src-tauri/deny.toml for the Rust side)."
  );
  process.exit(1);
}

console.log(`License gate passed (${packages.length} production packages checked).`);
