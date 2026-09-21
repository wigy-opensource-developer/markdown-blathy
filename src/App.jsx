import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { createPreviewFreeze } from "./lib/previewFreeze.js";
import { isStaleResponse } from "./lib/seq.js";
import { applyOptionsPatch, loadSettings, saveSettings } from "./lib/settings.js";
import { SAMPLE_DOC } from "./lib/sample.js";
import { docTitle, parentDirectory, safeFileName, windowTitle } from "./lib/title.js";
import Toolbar from "./components/Toolbar.jsx";
import Editor from "./components/Editor.jsx";
import Preview from "./components/Preview.jsx";
import StatusBar from "./components/StatusBar.jsx";
import ErrorBanner from "./components/ErrorBanner.jsx";

export default function App() {
  // Settings are loaded once from localStorage (M5, §8.6) and are the single
  // source of truth for the toolbar options; every option change persists
  // through `changeOption`. `marginPreset` is UI-only and stripped when the
  // payload is built (see lib/api.js); the rest mirrors RenderOptions (§6.1).
  const [settings, setSettings] = useState(() => loadSettings(window.localStorage));
  const { options, editorFontSize } = settings;
  const [markdown, setMarkdown] = useState(SAMPLE_DOC);
  const [pages, setPages] = useState([]);
  const [warnings, setWarnings] = useState([]);
  const [error, setError] = useState(null);
  // The current document's path on disk; null while untitled (§8.4). State,
  // not a ref, so the derived title updates when it changes.
  const [filePath, setFilePath] = useState(null);
  // The directory the dialogs start browsing in: `$XDG_DATA_HOME`/`$HOME` at
  // startup, then the directory of the most recently opened file (§8.4).
  const [directory, setDirectory] = useState(null);
  // Last preview request sequence we *sent*; responses older than this are
  // dropped (§7.5).
  const seq = useRef(0);
  // The preview freeze, built below once the render callback it wraps exists
  // (§6.2; see lib/previewFreeze.js).
  const previewFreeze = useRef(null);

  // The effective document settings from the last render (§6.2): Rust parses
  // the front matter and returns it, so the frontend never parses YAML
  // itself. Null until the first render lands.
  const [docMeta, setDocMeta] = useState(null);

  // True while the Document settings popover is open: the app then treats it
  // as a modal — the editor/preview/status are blurred, `inert` (for real:
  // clicks, caret and screen readers stay out) and frozen (see `.settings-open`
  // in styles.css and lib/previewFreeze.js).
  const [settingsOpen, setSettingsOpen] = useState(false);

  // The document title: the Rust-parsed frontmatter `title` wins, then the
  // file stem, then "Sample" for the (still untitled) sample document
  // (§8.4). Derived from `docMeta` + `filePath`, so it refreshes whenever a
  // render lands or the file changes — naturally on the same 200 ms debounce
  // as the preview, with no client-side frontmatter parsing.
  const title = useMemo(
    () => docTitle(docMeta?.title ?? null, filePath, "Sample"),
    [docMeta, filePath]
  );

  /** The pages of a finished render land in one (heavy) React commit. */
  const applyPreview = useCallback((result) => {
    setPages(result.pages);
    setWarnings(result.warnings);
    setDocMeta(result.doc);
    setError(null);
  }, []);

  // Debounced preview refresh: pauses 200ms after the last keystroke, well
  // within the ≤300ms acceptance for M3. `useDebouncedCallback` keeps the
  // latest callback in a ref, so `markdown`/`options` are always fresh when
  // the debounced call actually fires. The returned function is stable across
  // renders, so effects/callbacks depending on it stay stable too.
  const refreshPreview = useDebouncedCallback(async () => {
    const freeze = previewFreeze.current;
    freeze.started();
    const seqId = ++seq.current;
    try {
      const result = await api.renderPreview(markdown, options, seqId);
      if (isStaleResponse(result.seq, seq.current)) return; // stale — drop
      freeze.finish(result); // commit now, or stash while settings are open
    } catch (err) {
      setError(String(err)); // keep the last good preview
    }
  }, 200);

  // Freezes document renders while the Document settings popover is open
  // (§6.2): rendering blocks the UI thread and scales with the document, so
  // settings edits must not race a render scheduled by earlier editor
  // keystrokes; changes made meanwhile collapse into one catch-up render on
  // close. lib/previewFreeze.js owns that ordering; the App only reports
  // render lifecycle (`started`/`finish`) and the popover open/close. The
  // cycle — refreshPreview uses the freeze, the freeze re-schedules
  // refreshPreview — resolves through the ref: both exist before either is
  // called, and all three captured functions are stable across renders.
  if (!previewFreeze.current) {
    previewFreeze.current = createPreviewFreeze({
      refresh: refreshPreview,
      cancel: refreshPreview.cancel,
      apply: applyPreview,
    });
  }

  /** Schedule a preview; deferred as owed while the popover is open. */
  const requestPreview = useCallback(() => previewFreeze.current.request(), []);

  /** Popover opened → freeze; closed → the freeze catches up once. */
  const handleSettingsOpenChange = useCallback((open) => {
    setSettingsOpen(open);
    if (open) previewFreeze.current.open();
    else previewFreeze.current.close();
  }, []);

  // Initial render of the starting document.
  useEffect(() => {
    requestPreview();
    return () => refreshPreview.cancel();
  }, [requestPreview, refreshPreview]);

  // Load the startup working directory ($XDG_DATA_HOME / $HOME) for the
  // dialogs; until it resolves the dialogs fall back to the OS default.
  useEffect(() => {
    api
      .getInitialDirectory()
      .then((dir) => {
        if (dir) setDirectory(dir);
      })
      .catch(() => {});
  }, []);

  // Keep the OS window title in sync with the document title (§8.4) so
  // multiple instances are distinguishable in the window manager.
  useEffect(() => {
    api.setWindowTitle(windowTitle(title)).catch(() => {});
  }, [title]);

  const handleEditorChange = useCallback(
    (value) => {
      setMarkdown(value);
      requestPreview();
    },
    [requestPreview]
  );

  // Apply a partial options patch; switching the margin preset maps it to
  // millimetre margins. `custom` keeps the current margins (custom margin
  // editing arrives in a later milestone). Every change persists to
  // localStorage so the settings survive a restart (M5, §8.6). The patch is
  // merged through applyOptionsPatch, which drops the null a Mantine Select
  // emits when the selected option is re-clicked, so a deselect can't corrupt
  // the options (§13).
  const changeOption = useCallback(
    (patch) => {
      const nextOptions = applyOptionsPatch(settings.options, patch);
      if (patch.marginPreset) {
        nextOptions.margins =
          marginsForPreset(patch.marginPreset) ?? settings.options.margins;
      }
      const next = { ...settings, options: nextOptions };
      setSettings(next);
      saveSettings(next, window.localStorage);
      requestPreview();
    },
    [settings, requestPreview]
  );

  // Every file/template/link handler shares one error path: surface the
  // message in the banner and keep the last good state (§9). Wrapping the
  // action here removes the repeated try/catch from each handler below.
  const runFileAction = useCallback(async (action) => {
    try {
      await action();
    } catch (err) {
      setError(String(err));
    }
  }, []);

  // File handlers stay thin: they call the tested `lib/api.js` wrappers and
  // map the outcome onto state. A dismissed dialog is silent (§9); any other
  // failure surfaces in the error banner. All are stable so the memoized
  // toolbar does not re-render on every keystroke.
  const handleNew = useCallback(() => {
    setFilePath(null);
    setMarkdown(SAMPLE_DOC);
    requestPreview();
  }, [requestPreview]);

  const handleOpen = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.openMarkdown(directory);
        if (api.isCanceled(res)) return;
        setFilePath(res.path);
        setMarkdown(res.content);
        setDirectory(parentDirectory(res.path));
        requestPreview();
      }),
    [runFileAction, directory, requestPreview]
  );

  // Always a "Save As…" dialog: the filename defaults to `<title>.md` in the
  // working Directory (§8.4). The returned path becomes the new filePath, so
  // the title picks up the new stem when there is no frontmatter.
  const handleSave = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.saveMarkdown(markdown, safeFileName(title), directory);
        if (api.isCanceled(res)) return;
        setFilePath(res.path);
      }),
    [runFileAction, markdown, title, directory]
  );

  // Template picker (M5, §8.4): choose a `.typ` file via the system dialog,
  // validate it with a probe compile, and only apply it on success — a broken
  // template keeps the previous one (or the embedded default) and surfaces
  // the diagnostics in the error banner. A dismissed dialog is silent (§9).
  const handlePickTemplate = useCallback(
    () =>
      runFileAction(async () => {
        const picked = await open({
          title: "Choose a Typst template",
          multiple: false,
          filters: [{ name: "Typst template", extensions: ["typ"] }],
          defaultPath: directory ?? undefined,
        });
        if (picked === null) return; // canceled — silent
        await api.validateTemplate(picked, options);
        changeOption({ templatePath: picked });
      }),
    [runFileAction, directory, options, changeOption]
  );

  // Clicking the active-template badge drops it again — back to the embedded
  // default, as if no template was ever loaded (persisted like any option).
  const handleClearTemplate = useCallback(() => {
    changeOption({ templatePath: null });
  }, [changeOption]);

  // Document settings popover (§6.2): apply a front-matter patch through the
  // Rust editor, which preserves unmanaged keys, then re-render the result.
  // Requesting while the popover is still open defers the render to its close
  // (`handleSettingsOpenChange`), so the document is not re-rendered under the
  // user's hands.
  const handleApplySettings = useCallback(
    (patch) =>
      runFileAction(async () => {
        const updated = await api.updateFrontMatter(markdown, patch);
        setMarkdown(updated);
        requestPreview();
      }),
    [runFileAction, markdown, requestPreview]
  );

  const handleExport = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.exportPdf(markdown, options, safeFileName(title), directory);
        if (api.isCanceled(res)) return;
        // The PDF is written and opened in the system viewer by the backend.
      }),
    [runFileAction, markdown, options, title, directory]
  );

  const dismissError = useCallback(() => setError(null), []);

  // Preview links are intercepted in Preview.jsx and forwarded here: http(s)
  // opens in the system browser, anything else is rejected by the backend
  // with an error (§7.4 / §9) — the webview never navigates to a link.
  const handleExternalLink = useCallback(
    (url) => runFileAction(() => api.openExternal(url)),
    [runFileAction]
  );

  return (
    <div className={settingsOpen ? "app settings-open" : "app"}>
      <Toolbar
        options={options}
        onOptionChange={changeOption}
        onNew={handleNew}
        onOpen={handleOpen}
        onSave={handleSave}
        onPickTemplate={handlePickTemplate}
        onClearTemplate={handleClearTemplate}
        onExport={handleExport}
        docMeta={docMeta}
        onApplySettings={handleApplySettings}
        onSettingsOpenChange={handleSettingsOpenChange}
      />
      <main id="panes" inert={settingsOpen}>
        <section id="editor-pane">
          <Editor
            value={markdown}
            onChange={handleEditorChange}
            onSave={handleSave}
            fontSize={editorFontSize}
          />
        </section>
        <section id="preview-pane">
          <Preview pages={pages} onExternalLink={handleExternalLink} />
        </section>
      </main>
      <StatusBar
        pageCount={pages.length}
        warnings={warnings}
        inert={settingsOpen}
      />
      <ErrorBanner message={error} onClose={dismissError} />
    </div>
  );
}
