import { useCallback, useEffect, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { isStaleResponse } from "./lib/seq.js";
import { loadSettings, saveSettings } from "./lib/settings.js";
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

  // The document title: frontmatter `title` wins, then the file stem, then
  // "Sample" for the (still untitled) sample document (§8.4). It is *state*,
  // refreshed on the same 200 ms debounce as the preview, so fast typing in
  // the frontmatter never parses the document per keystroke and the
  // window-title IPC update lands at most once per debounce period.
  const [title, setTitle] = useState(() => docTitle(SAMPLE_DOC, null, "Sample"));

  // Debounced preview refresh: pauses 200ms after the last keystroke, well
  // within the ≤300ms acceptance for M3. `useDebouncedCallback` keeps the
  // latest callback in a ref, so `markdown`/`options` are always fresh when
  // the debounced call actually fires. The returned function is stable across
  // renders, so effects/callbacks depending on it stay stable too.
  const refreshPreview = useDebouncedCallback(async () => {
    const seqId = ++seq.current;
    try {
      const result = await api.renderPreview(markdown, options, seqId);
      if (isStaleResponse(result.seq, seq.current)) return; // stale — drop
      setPages(result.pages);
      setWarnings(result.warnings);
      setError(null);
    } catch (err) {
      setError(String(err)); // keep the last good preview
    }
  }, 200);

  // Title derivation (frontmatter parsing) rides the same debounce as the
  // preview (§8.4): the parsed title only lands 200 ms after typing pauses.
  const refreshTitle = useDebouncedCallback(() => {
    setTitle(docTitle(markdown, filePath, "Sample"));
  }, 200);

  // Initial render of the starting document.
  useEffect(() => {
    refreshPreview();
    return () => refreshPreview.cancel();
  }, [refreshPreview]);

  // Cancel a pending title refresh on unmount (no setState after unmount).
  useEffect(() => () => refreshTitle.cancel(), [refreshTitle]);

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
      refreshPreview();
      refreshTitle();
    },
    [refreshPreview, refreshTitle]
  );

  // Apply a partial options patch; switching the margin preset maps it to
  // millimetre margins. `custom` keeps the current margins (custom margin
  // editing arrives in a later milestone). Every change persists to
  // localStorage so the settings survive a restart (M5, §8.6).
  const changeOption = useCallback(
    (patch) => {
      const nextOptions = { ...settings.options, ...patch };
      if (patch.marginPreset) {
        nextOptions.margins =
          marginsForPreset(patch.marginPreset) ?? settings.options.margins;
      }
      const next = { ...settings, options: nextOptions };
      setSettings(next);
      saveSettings(next, window.localStorage);
      refreshPreview();
    },
    [settings, refreshPreview]
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
    refreshPreview();
    refreshTitle();
  }, [refreshPreview, refreshTitle]);

  const handleOpen = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.openMarkdown(directory);
        if (api.isCanceled(res)) return;
        setFilePath(res.path);
        setMarkdown(res.content);
        setDirectory(parentDirectory(res.path));
        refreshPreview();
        refreshTitle();
      }),
    [runFileAction, directory, refreshPreview, refreshTitle]
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
        refreshTitle();
      }),
    [runFileAction, markdown, title, directory, refreshTitle]
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
    <div className="app">
      <Toolbar
        options={options}
        onOptionChange={changeOption}
        onNew={handleNew}
        onOpen={handleOpen}
        onSave={handleSave}
        onPickTemplate={handlePickTemplate}
        onClearTemplate={handleClearTemplate}
        onExport={handleExport}
      />
      <main id="panes">
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
      <StatusBar pageCount={pages.length} warnings={warnings} />
      <ErrorBanner message={error} onClose={dismissError} />
    </div>
  );
}
