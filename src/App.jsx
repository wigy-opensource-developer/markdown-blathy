import { useCallback, useEffect, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { isStaleResponse } from "./lib/seq.js";
import Toolbar from "./components/Toolbar.jsx";
import Editor from "./components/Editor.jsx";
import Preview from "./components/Preview.jsx";
import StatusBar from "./components/StatusBar.jsx";
import ErrorBanner from "./components/ErrorBanner.jsx";

const INITIAL_DOC = `# Hello, Markdown editor

Start typing in the editor pane. The live preview and PDF export are
powered by a Typst compilation pipeline running on the Rust side.
`;

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
  const [markdown, setMarkdown] = useState(INITIAL_DOC);
  const [options, setOptions] = useState(DEFAULT_OPTIONS);
  const [pages, setPages] = useState([]);
  const [warnings, setWarnings] = useState([]);
  const [error, setError] = useState(null);
  // Last preview request sequence we *sent*; responses older than this are
  // dropped (§7.5).
  const seq = useRef(0);

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

  // Initial render of the starting document.
  useEffect(() => {
    refreshPreview();
    return () => refreshPreview.cancel();
  }, [refreshPreview]);

  const handleEditorChange = useCallback(
    (value) => {
      setMarkdown(value);
      refreshPreview();
    },
    [refreshPreview]
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

  // The current document's path on disk; null while untitled. Set by Open and
  // Save; Save reads it so already-named documents skip the dialog (§8.4).
  const filePath = useRef(null);

  // File handlers stay thin: they call the tested `lib/api.js` wrappers and
  // map the outcome onto state. A dismissed dialog is silent (§9); any other
  // failure surfaces in the error banner. All are stable so the memoized
  // toolbar does not re-render on every keystroke.
  const handleNew = useCallback(() => {
    filePath.current = null;
    setMarkdown(INITIAL_DOC);
    refreshPreview();
  }, [refreshPreview]);

  const handleOpen = useCallback(async () => {
    try {
      const res = await api.openMarkdown();
      if (api.isCanceled(res)) return;
      filePath.current = res.path;
      setMarkdown(res.content);
      refreshPreview();
    } catch (err) {
      setError(String(err));
    }
  }, [refreshPreview]);

  const handleSave = useCallback(async () => {
    try {
      const res = await api.saveMarkdown(filePath.current, markdown);
      if (api.isCanceled(res)) return;
      filePath.current = res.path;
    } catch (err) {
      setError(String(err));
    }
  }, [markdown]);

  const handlePickTemplate = useCallback(() => {}, []); // M5

  const handleExport = useCallback(async () => {
    try {
      const res = await api.exportPdf(markdown, options);
      if (api.isCanceled(res)) return;
      // The PDF is written and opened in the system viewer by the backend.
    } catch (err) {
      setError(String(err));
    }
  }, [markdown, options]);

  const dismissError = useCallback(() => setError(null), []);

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
          <Preview pages={pages} />
        </section>
      </main>
      <StatusBar pageCount={pages.length} warnings={warnings} />
      <ErrorBanner message={error} onClose={dismissError} />
    </div>
  );
}
