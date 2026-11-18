// Thin invoke() wrappers for the Tauri commands (§8.4 of SPEC.md).
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { renderOptionsPayload } from "./presets.js";

/** Live preview: markdown + options → per-page SVG strings. The `seq` is
 *  echoed back for the stale-response guard (§7.5). `documentPath` lets the
 *  backend resolve document-relative images against the document's folder. */
export const renderPreview = (markdown, options, seq, documentPath = null) =>
  invoke("render_preview", {
    payload: {
      markdown,
      options: renderOptionsPayload(options),
      seq,
      documentPath,
    },
  });

/** Export PDF: markdown + options → system save dialog (defaulting to
 *  `<title>.pdf` in `directory`) → PDF on disk, opened in the system viewer.
 *  Resolves with `{ path, warnings? }` or `{ canceled: true }` (§7.4, M9
 *  task 5); `warnings` lists compile warnings the UI shows in a dismissible
 *  panel. `seq` guards preview staleness and is unused here. */
export const exportPdf = (markdown, options, title, directory, documentPath = null) =>
  invoke("export_pdf", {
    payload: {
      markdown,
      options: renderOptionsPayload(options),
      seq: 0,
      documentPath,
    },
    title,
    directory,
  });

/** Open a Markdown file via the system dialog, starting in `directory`
 *  (null lets the OS pick). Resolves with `{ path, content }` or
 *  `{ canceled: true }` (§7.4). */
export const openMarkdown = (directory) =>
  invoke("open_markdown", { directory });

/** Take the document the CLI asked to open (M9). Called after the dirty
 *  guard passes; the backend reads its exact `PathBuf` and remembers it for
 *  save-in-place. Resolves with `{ path, content }` or `{ canceled: true }`
 *  when the request was already consumed. */
export const openPendingDocument = () => invoke("open_pending_document");

/** Announce that the `app-request` listener is registered (M9): the backend
 *  then emits the pending open request. Resolves to `true` when one was sent
 *  — the caller then skips the startup recovery check (the open completes
 *  first and runs its own check). Safe to call more than once. */
export const frontendReady = () => invoke("frontend_ready");

/** Approve the close the frontend just guarded (M9): the backend arms a
 *  one-shot allowance and closes the window. */
export const confirmClose = () => invoke("confirm_close");

/** Save Markdown via a "Save As…" dialog: filename defaults to `<title>.md`
 *  in `directory`. Resolves with `{ path }` or `{ canceled: true }` (§7.4).
 *  The chosen path becomes the backend's save-in-place target. */
export const saveMarkdown = (content, title, directory) =>
  invoke("save_markdown", { content, title, directory });

/** Save the open document in place (M9): writes to the exact `PathBuf` the
 *  backend remembers from the last open/Save As, without a dialog. The path
 *  never travels through this IPC string, so a non-UTF-8 filename still
 *  saves to the right file (§6). Rejects when the document is untitled. */
export const saveDocument = (content) => invoke("save_document", { content });

/** Forget the backend's save-in-place path (New): a later plain save must not
 *  overwrite the previously opened file. */
export const forgetDocument = () => invoke("forget_document");

/** The working directory at startup: `$XDG_DATA_HOME` or `$HOME` (§8.4). */
export const getInitialDirectory = () => invoke("get_initial_directory");

/** Set the OS window title (shown in the window manager / taskbar). The
 *  caller composes the full string, e.g. via `windowTitle()` (§8.4). */
export const setWindowTitle = (title) => getCurrentWindow().setTitle(title);

/** Open an http(s) URL in the system default browser. Non-http(s) URLs
 *  are rejected by the backend with an error — the webview never navigates
 *  (§7.4 / §9). */
export const openExternal = (url) => invoke("open_external", { url });

/** Update the document's front matter with a settings patch and resolve with
 *  the full new Markdown (§6.2). Rust owns the YAML editing: managed keys are
 *  rewritten canonically, unknown keys preserved. An omitted key leaves a
 *  value untouched; `title`/`author`/`lang` accept `""` to clear, and
 *  `hyphenate` clears with an explicit `null` (absent = keep, since `false`
 *  is a meaningful value). */
export const updateFrontMatter = (markdown, patch) =>
  invoke("update_front_matter", { markdown, patch });

/** Validate a custom `.typ` template by probe-compiling it (§7.4, M5/M6). *  The current options are sent along so validation builds the exact engine
 *  the next render will use — and hands it to the cache, making the first
 *  render instant. Resolves with `{ valid: true }`; rejects with the
 *  offending path and the compile diagnostics when the template is broken. */
export const validateTemplate = (path, options) =>
  invoke("validate_template", {
    path,
    options: renderOptionsPayload(options),
  });

/** List the bundled template ids (e.g. `light`, `dark`, `obsidian`), ordered
 *  with the default `light` first. The frontend maps ids to display labels
 *  via `templateLabel()`. */
export const listTemplates = () => invoke("list_templates");

/** Autosave (M9 task 4): write the recovery snapshot for the open document.
 *  The backend uses its exact `PathBuf` as the slot key; best-effort. */
export const saveRecoverySnapshot = (markdown) =>
  invoke("save_recovery_snapshot", { markdown });

/** The newest recovery snapshot worth offering, or null. Does not delete it. */
export const takeRecovery = () => invoke("take_recovery");

/** Restore a snapshot: resolves with `{ markdown, path }` and points
 *  save-in-place at the exact original file. */
export const restoreRecovery = (id) => invoke("restore_recovery", { id });

/** Delete a snapshot after the user chose Discard. */
export const discardRecovery = (id) => invoke("discard_recovery", { id });

/** Drop the current document's snapshot (and the untitled one) — called when
 *  the user discards unsaved changes. */
export const clearRecovery = () => invoke("clear_recovery");

/** True when a command result means the user dismissed a dialog — the UI
 *  treats these as silent no-ops (§9). */
export const isCanceled = (result) => result?.canceled === true;
