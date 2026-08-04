// Thin invoke() wrappers for the Tauri commands (§8.4 of SPEC.md).
import { invoke } from "@tauri-apps/api/core";
import { renderOptionsPayload } from "./presets.js";

/** Live preview: markdown + options → per-page SVG strings. The `seq` is
 *  echoed back for the stale-response guard (§7.5). */
export const renderPreview = (markdown, options, seq) =>
  invoke("render_preview", {
    payload: {
      markdown,
      options: renderOptionsPayload(options),
      seq,
    },
  });

/** Export PDF: markdown + options → system save dialog → PDF on disk, opened
 *  in the system viewer. Resolves with `{ path }` or `{ canceled: true }`
 *  (§7.4). `seq` guards preview staleness and is unused here. */
export const exportPdf = (markdown, options) =>
  invoke("export_pdf", {
    payload: {
      markdown,
      options: renderOptionsPayload(options),
      seq: 0,
    },
  });

/** Open a Markdown file via the system dialog. Resolves with
 *  `{ path, content }` or `{ canceled: true }` (§7.4). */
export const openMarkdown = () => invoke("open_markdown");

/** Save Markdown to `path`, or show the system save dialog when `path` is
 *  null (untitled document). Resolves with `{ path }` or `{ canceled: true }`. */
export const saveMarkdown = (path, content) =>
  invoke("save_markdown", { path, content });

/** True when a command result means the user dismissed a dialog — the UI
 *  treats these as silent no-ops (§9). */
export const isCanceled = (result) => result?.canceled === true;
