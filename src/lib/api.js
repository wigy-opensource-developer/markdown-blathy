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
