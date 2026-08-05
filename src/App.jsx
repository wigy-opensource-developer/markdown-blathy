import { useCallback, useEffect, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { isStaleResponse } from "./lib/seq.js";
import { SAMPLE_DOC } from "./lib/sample.js";
import { docTitle, parentDirectory, safeFileName, windowTitle } from "./lib/title.js";
import Toolbar from "./components/Toolbar.jsx";
import Editor from "./components/Editor.jsx";
import Preview from "./components/Preview.jsx";
import StatusBar from "./components/StatusBar.jsx";
import ErrorBanner from "./components/ErrorBanner.jsx";

// Frontend options state. `marginPreset` is UI-only and stripped when the
// payload is built (see lib/api.js); the rest mirrors RenderOptions (§6.1).
const DEFAULT_OPTIONS = {
  flavor: "gfm",
  paper: "a4",
  marginPreset: "normal",
  margins: marginsForPreset("normal"),
  textAlign: "left",
  templatePath: null,
};

export default function App() {
  const [markdown, setMarkdown] = useState(SAMPLE_DOC);
  const [options, setOptions] = useState(DEFAULT_OPTIONS);
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
  // editing arrives in a later milestone).
  const changeOption = useCallback(
    (patch) => {
      setOptions((prev) => {
        const margins = patch.marginPreset
          ? marginsForPreset(patch.marginPreset) ?? prev.margins
          : prev.margins;
        return { ...prev, ...patch, margins };
      });
      refreshPreview();
    },
    [refreshPreview]
  );

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

  const handleOpen = useCallback(async () => {
    try {
      const res = await api.openMarkdown(directory);
      if (api.isCanceled(res)) return;
      setFilePath(res.path);
      setMarkdown(res.content);
      setDirectory(parentDirectory(res.path));
      refreshPreview();
      refreshTitle();
    } catch (err) {
      setError(String(err));
    }
  }, [directory, refreshPreview, refreshTitle]);

  // Always a "Save As…" dialog: the filename defaults to `<title>.md` in the
  // working Directory (§8.4). The returned path becomes the new filePath, so
  // the title picks up the new stem when there is no frontmatter.
  const handleSave = useCallback(async () => {
    try {
      const res = await api.saveMarkdown(markdown, safeFileName(title), directory);
      if (api.isCanceled(res)) return;
      setFilePath(res.path);
      refreshTitle();
    } catch (err) {
      setError(String(err));
    }
  }, [markdown, title, directory, refreshTitle]);

  const handlePickTemplate = useCallback(() => {}, []); // M5

  const handleExport = useCallback(async () => {
    try {
      const res = await api.exportPdf(markdown, options, safeFileName(title), directory);
      if (api.isCanceled(res)) return;
      // The PDF is written and opened in the system viewer by the backend.
    } catch (err) {
      setError(String(err));
    }
  }, [markdown, options, title, directory]);

  const dismissError = useCallback(() => setError(null), []);

  // Preview links are intercepted in Preview.jsx and forwarded here: http(s)
  // opens in the system browser, anything else is rejected by the backend
  // with an error (§7.4 / §9) — the webview never navigates to a link.
  const handleExternalLink = useCallback(async (url) => {
    try {
      await api.openExternal(url);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  return (
    <div className="app">
      <Toolbar
        options={options}
        onOptionChange={changeOption}
        onNew={handleNew}
        onOpen={handleOpen}
        onSave={handleSave}
        onPickTemplate={handlePickTemplate}
        onExport={handleExport}
      />
      <main id="panes">
        <section id="editor-pane">
          <Editor value={markdown} onChange={handleEditorChange} onSave={handleSave} />
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
