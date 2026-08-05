// Thin invoke() wrappers for the Tauri commands (§8.4 of SPEC.md).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
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

/** Export PDF: markdown + options → system save dialog (defaulting to
 *  `<title>.pdf` in `directory`) → PDF on disk, opened in the system viewer.
 *  Resolves with `{ path }` or `{ canceled: true }` (§7.4). `seq` guards
 *  preview staleness and is unused here. */
export const exportPdf = (markdown, options, title, directory) =>
  invoke("export_pdf", {
    payload: {
      markdown,
      options: renderOptionsPayload(options),
      seq: 0,
    },
    title,
    directory,
  });

/** Open a Markdown file via the system dialog, starting in `directory`
 *  (null lets the OS pick). Resolves with `{ path, content }` or
 *  `{ canceled: true }` (§7.4). */
export const openMarkdown = (directory) =>
  invoke("open_markdown", { directory });

/** Save Markdown via a "Save As…" dialog: filename defaults to `<title>.md`
 *  in `directory`. Resolves with `{ path }` or `{ canceled: true }` (§7.4). */
export const saveMarkdown = (content, title, directory) =>
  invoke("save_markdown", { content, title, directory });

/** The working directory at startup: `$XDG_DATA_HOME` or `$HOME` (§8.4). */
export const getInitialDirectory = () => invoke("get_initial_directory");

/** Set the OS window title (shown in the window manager / taskbar). The
 *  caller composes the full string, e.g. via `windowTitle()` (§8.4). */
export const setWindowTitle = (title) => getCurrentWindow().setTitle(title);

/** Open an http(s) URL in the system default browser. Non-http(s) URLs are
 *  rejected by the backend with an error — the webview never navigates to
 *  them (§7.4 / §9). */
export const openExternal = (url) => invoke("open_external", { url });

/** True when a command result means the user dismissed a dialog — the UI
 *  treats these as silent no-ops (§9). */
export const isCanceled = (result) => result?.canceled === true;
