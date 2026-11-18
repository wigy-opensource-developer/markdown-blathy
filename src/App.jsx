import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useDebouncedCallback } from "@mantine/hooks";
import { MantineProvider } from "@mantine/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { createPreviewFreeze } from "./lib/previewFreeze.js";
import { createRenderStatus } from "./lib/renderStatus.js";
import { isStaleResponse } from "./lib/seq.js";
import { getCommand } from "./lib/commands.js";
import { handleKeydown } from "./lib/keyboard.js";
import { detectPlatform } from "./lib/platform.js";
import { applyDocSettingsPatch } from "./lib/docSettings.js";
import { applyOptionsPatch, loadSettings, saveSettings } from "./lib/settings.js";
import { mantineTheme } from "./lib/mantineTheme.js";
import { resolveColorScheme } from "./lib/uiTheme.js";
import { SAMPLE_DOC } from "./lib/sample.js";
import { docTitle, parentDirectory, safeFileName, windowTitle } from "./lib/title.js";
import { createUnsavedGuard } from "./lib/unsavedGuard.js";
import Toolbar from "./components/Toolbar.jsx";
import Editor from "./components/Editor.jsx";
import Preview from "./components/Preview.jsx";
import StatusBar from "./components/StatusBar.jsx";
import ErrorBanner from "./components/ErrorBanner.jsx";
import UnsavedChangesModal from "./components/UnsavedChangesModal.jsx";

export default function App() {
  // Settings are loaded once from localStorage (M5, §8.6) and are the single
  // source of truth for the render options; every option change persists
  // through `changeOption`. `marginPreset` is UI-only and stripped when the
  // payload is built (see lib/api.js); the rest mirrors RenderOptions (§6.1).
  const [settings, setSettings] = useState(() => loadSettings(window.localStorage));
  const { options, editorFontSize, theme } = settings;
  // The OS color scheme, tracked live so "system" follows changes at runtime.
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false
  );
  // The concrete Mantine color scheme for the UI chrome only.
  const colorScheme = resolveColorScheme(theme, systemDark);
  const [markdown, setMarkdown] = useState(SAMPLE_DOC);
  // True while the document has edits that are not on disk (M9): drives the
  // window-title marker and the unsaved-changes guard. A ref mirrors it for
  // the guard, whose callbacks outlive a render.
  const [dirty, setDirty] = useState(false);
  const dirtyRef = useRef(false);
  dirtyRef.current = dirty;
  const [pages, setPages] = useState([]);
  const [warnings, setWarnings] = useState([]);
  const [error, setError] = useState(null);
  // Bundled template ids for the Page settings selector; seeded with the
  // known trio and refreshed from `list_templates` (graceful on failure).
  const [templates, setTemplates] = useState(["light", "dark", "obsidian"]);
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
  // The delayed "Rendering…" state machine (lib/renderStatus.js), built
  // below with its React binding.
  const renderStatus = useRef(null);
  // The document the *next render* will use. Normally synced from state on
  // every render — but a freshly opened file is written here before calling
  // `requestPreviewNow`, because the immediate render runs before React has
  // committed the new `markdown`. `documentPath` lets the backend resolve
  // document-relative images.
  const docRef = useRef({ markdown: SAMPLE_DOC, options, documentPath: null });
  docRef.current = { markdown, options, documentPath: filePath };

  // The effective document settings from the last render (§6.2): Rust parses
  // the front matter and returns it, so the frontend never parses YAML
  // itself. Applied settings patches are merged in immediately (the canonical
  // value Rust just wrote), so the dialog never shows stale values while the
  // catch-up render is pending. Null until the first render lands.
  const [docMeta, setDocMeta] = useState(null);

  // Modals (the Document settings popover and the unsaved-changes prompt)
  // share one treatment: the editor/preview/status are blurred, `inert`
  // (clicks, caret and screen readers stay out) and frozen (see
  // `.modal-open` in styles.css and lib/previewFreeze.js). The counter keeps
  // two overlapping modals from unfreezing each other early.
  const [modalDepth, setModalDepth] = useState(0);
  const modalDepthRef = useRef(0);
  const modalOpen = modalDepth > 0;
  // True while the Document settings popover is open (counted in modalDepth).
  const [settingsOpen, setSettingsOpen] = useState(false);
  const settingsOpenRef = useRef(false);
  // The unsaved-changes prompt (M9 task 3): open while the guard holds a
  // request; `confirmBusy` covers a Save that opens the Save As dialog.
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmBusy, setConfirmBusy] = useState(false);
  // Command-controlled popover/menu state: the registry opens them, their own
  // targets toggle them, and Mantine reports outside clicks/Escape back here.
  const [pageSettingsOpen, setPageSettingsOpen] = useState(false);
  const [themeMenuOpen, setThemeMenuOpen] = useState(false);
  // Bumped by file.new/file.open so the editor can take focus after the new
  // document has committed.
  const [editorFocusToken, setEditorFocusToken] = useState(0);
  const platform = useMemo(() => detectPlatform(), []);

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
    renderStatus.current.stop(); // the pending note comes down with the pages
  }, []);

  // Delayed "Rendering…" feedback on the preview pane: keystroke renders
  // usually land before the note even arms (350 ms), so it never flickers;
  // big documents do pend long enough to show it. lib/renderStatus.js owns
  // the timing; this binding just mirrors visibility into React state.
  const [renderBusy, setRenderBusy] = useState(false);
  if (!renderStatus.current) {
    renderStatus.current = createRenderStatus({ delayMs: 350, notify: setRenderBusy });
  }

  // Debounced preview refresh: pauses 200ms after the last keystroke, well
  // within the ≤300ms acceptance for M3. `useDebouncedCallback` keeps the
  // latest callback in a ref, so `markdown`/`options` are always fresh when
  // the debounced call actually fires. The returned function is stable across
  // renders, so effects/callbacks depending on it stay stable too.
  // The render body: reads `docRef`, so a `requestPreviewNow()` fired in the
  // same event as `setMarkdown(...)` still renders the new text, and the
  // debounced path renders whatever is current when the timer fires. All its
  // captures are refs/stable setters, so this callback itself is stable.
  const runRender = useCallback(async () => {
    const freeze = previewFreeze.current;
    freeze.started();
    renderStatus.current.start();
    const seqId = ++seq.current;
    const { markdown: doc, options: opts, documentPath: docPath } = docRef.current;
    try {
      const result = await api.renderPreview(doc, opts, seqId, docPath);
      if (isStaleResponse(result.seq, seq.current)) return; // stale — drop (a newer render owns the note now)
      freeze.finish(result); // commit now, or stash while settings are open
    } catch (err) {
      renderStatus.current.stop();
      setError(String(err)); // keep the last good preview
    }
  }, []);

  // Debounced preview refresh: pauses 200ms after the last keystroke, well
  // within the ≤300ms acceptance for M3. Keystrokes are the only callers
  // that want to wait; a freshly loaded document asks for `requestNow`
  // (below) and starts rendering at once.
  const refreshPreview = useDebouncedCallback(runRender, 200);

  // Freezes document renders while the Document settings popover is open
  // (§6.2): rendering blocks the UI thread and scales with the document, so
  // settings edits must not race a render scheduled by earlier editor
  // keystrokes; changes made meanwhile collapse into one catch-up render on
  // close. lib/previewFreeze.js owns that ordering; the App only reports
  // render lifecycle (`started`/`finish`) and the popover open/close. The
  // cycle — refreshPreview uses the freeze, the freeze re-schedules
  // refreshPreview — resolves through the ref: both exist before either is
  // called, and all captured functions are stable across renders.
  if (!previewFreeze.current) {
    previewFreeze.current = createPreviewFreeze({
      refresh: refreshPreview,
      refreshNow: runRender,
      cancel: refreshPreview.cancel,
      apply: applyPreview,
    });
  }

  /** Schedule a preview; deferred as owed while the popover is open. */
  const requestPreview = useCallback(() => previewFreeze.current.request(), []);

  /**
   * Render a just-loaded document right away (no 200 ms debounce). `patch`
   * carries the fresh text when the caller's `setMarkdown(...)` has not
   * re-rendered yet — the initial sample needs no patch.
   */
  const requestPreviewNow = useCallback((patch) => {
    if (patch) docRef.current = { ...docRef.current, ...patch };
    previewFreeze.current.requestNow();
  }, []);

  /** Enter a modal interaction: the first one freezes previews and blurs the
   *  panes; the preview freeze catches up once when the last one closes. */
  const enterModal = useCallback(() => {
    if (modalDepthRef.current++ === 0) previewFreeze.current.open();
    setModalDepth(modalDepthRef.current);
  }, []);
  const leaveModal = useCallback(() => {
    if (modalDepthRef.current === 0) return; // unpaired notification
    if (--modalDepthRef.current === 0) previewFreeze.current.close();
    setModalDepth(modalDepthRef.current);
  }, []);

  // The latest save routine, for the guard's Save choice. Assigned once the
  // file handlers below exist; the guard only calls it on a user choice.
  const saveCurrentRef = useRef(async () => false);

  // One guard for every request that may lose edits (M9 task 3): Close, New
  // and Open all go through it. Created once — it reads live state through
  // refs — and mirrors its pending state into React for the modal.
  const guardRef = useRef(null);
  if (!guardRef.current) {
    guardRef.current = createUnsavedGuard({
      isDirty: () => dirtyRef.current,
      save: () => saveCurrentRef.current(),
      perform: (action) => action(),
      onChange: (pending) => {
        setConfirmOpen(pending);
        if (pending) enterModal();
        else leaveModal();
      },
    });
  }

  /** A choice in the unsaved-changes prompt; the buttons stay disabled while
   *  a Save runs (it may open the Save As dialog). */
  const chooseUnsaved = useCallback(async (choice) => {
    setConfirmBusy(true);
    try {
      await guardRef.current.choose(choice);
    } finally {
      setConfirmBusy(false);
    }
  }, []);

  /** Popover opened → freeze; closed → the freeze catches up once. */
  const handleSettingsOpenChange = useCallback(
    (open) => {
      // Mantine may report the same state twice (initial closed notification);
      // only real transitions may enter/leave the shared modal treatment.
      if (open === settingsOpenRef.current) return;
      settingsOpenRef.current = open;
      setSettingsOpen(open);
      if (open) enterModal();
      else leaveModal();
    },
    [enterModal, leaveModal]
  );

  // Initial render of the starting document: straight away — this is boot,
  // not a keystroke. (The Rust side prewarms the default engine meanwhile.)
  useEffect(() => {
    requestPreviewNow();
    return () => refreshPreview.cancel();
  }, [requestPreviewNow, refreshPreview]);

  // `app-request` carries every backend request that needs the dirty guard
  // (M9 task 3): a window close, or a document the CLI asked to open. The
  // handler reads the latest callbacks through a ref, so the listener is
  // registered once. Registration precedes `frontendReady()`; the startup
  // document is only consumed by the open command, so a listener torn down by
  // a remount cannot lose it — the next mount asks again.
  const requestHandlerRef = useRef(() => {});
  useEffect(() => {
    let unlisten = null;
    let cancelled = false;
    (async () => {
      try {
        unlisten = await listen("app-request", (event) => {
          requestHandlerRef.current(event.payload);
        });
        if (cancelled) {
          unlisten();
          unlisten = null;
          return;
        }
        await api.frontendReady();
      } catch (err) {
        if (!cancelled) setError(String(err));
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Load the startup working directory ($XDG_DATA_HOME / $HOME) for the
  // dialogs; until it resolves the dialogs fall back to the OS default.
  useEffect(() => {
    api
      .getInitialDirectory()
      .then((dir) => {
        // Do not clobber the directory of a startup/open document that
        // resolved while this request was in flight.
        if (dir) setDirectory((current) => current ?? dir);
      })
      .catch(() => {});
  }, []);

  // Refresh the bundled template list for the Page settings selector; the
  // seed above keeps the selector usable if the command is unavailable.
  useEffect(() => {
    api
      .listTemplates()
      .then((ids) => {
        if (Array.isArray(ids) && ids.length > 0) setTemplates(ids);
      })
      .catch(() => {});
  }, []);

  // Follow the OS light/dark preference while the theme is set to "system".
  useEffect(() => {
    if (!window.matchMedia) return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event) => setSystemDark(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  // Keep the OS window title in sync with the document title (§8.4) so
  // multiple instances are distinguishable in the window manager; the dot
  // marks unsaved changes (M9).
  useEffect(() => {
    api.setWindowTitle(windowTitle(title, dirty)).catch(() => {});
  }, [title, dirty]);

  const handleEditorChange = useCallback(
    (value) => {
      setMarkdown(value);
      setDirty(true);
      requestPreview();
    },
    [requestPreview]
  );

  // Apply a partial options patch; switching the margin preset maps it to
  // millimetre margins. An unknown preset value keeps the current margins.
  // Every change persists to localStorage so the settings survive a restart
  // (M5, §8.6). The patch is
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
  // Open a document result into the editor state (dialog or CLI request).
  const openDocument = useCallback(
    (res) => {
      setFilePath(res.path);
      setMarkdown(res.content);
      setDirty(false);
      setDirectory(parentDirectory(res.path));
      requestPreviewNow({ markdown: res.content, documentPath: res.path });
    },
    [requestPreviewNow]
  );

  // New document: clear the backend's save-in-place path first. Awaiting it
  // means a failure keeps the old document (and its path) rather than leaving
  // the UI untitled while a plain save would still target the old file.
  const doNew = useCallback(
    () =>
      runFileAction(async () => {
        await api.forgetDocument();
        setFilePath(null);
        setMarkdown(SAMPLE_DOC);
        setDirty(false);
        requestPreviewNow({ markdown: SAMPLE_DOC, documentPath: null });
      }),
    [runFileAction, requestPreviewNow]
  );

  const doOpen = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.openMarkdown(directory);
        if (api.isCanceled(res)) return;
        openDocument(res);
      }),
    [runFileAction, directory, openDocument]
  );

  // The CLI's startup file arrives as an `app-request` open (M9 task 3); the
  // same dirty guard applies, so a forwarded document cannot replace edits
  // silently.
  const doOpenPending = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.openPendingDocument();
        if (api.isCanceled(res)) return;
        openDocument(res);
      }),
    [runFileAction, openDocument]
  );

  const doClose = useCallback(() => {
    api.confirmClose().catch((err) => setError(String(err)));
  }, []);

  // Requests from the backend go through the same guard as the toolbar.
  requestHandlerRef.current = (payload) => {
    if (payload?.kind === "close") guardRef.current.request(doClose);
    else if (payload?.kind === "open") guardRef.current.request(doOpenPending);
  };

  // New/Open are guarded: a dirty document asks Save/Discard/Cancel first
  // (M9 task 3), and a successful save resumes the interrupted request.
  const handleNew = useCallback(() => guardRef.current.request(doNew), [doNew]);
  const handleOpen = useCallback(() => guardRef.current.request(doOpen), [doOpen]);

  // Save As (Mod+Shift+S): always a dialog. The filename default comes from
  // the front-matter title; the chosen path becomes the backend's
  // save-in-place target. Returns whether the document reached disk, so the
  // guard can resume the request it interrupted (`false` when cancelled or
  // failed — the prompt stays open for a retry).
  const saveAsDialog = useCallback(async () => {
    try {
      const res = await api.saveMarkdown(
        docRef.current.markdown,
        safeFileName(title),
        directory
      );
      if (api.isCanceled(res)) return false;
      setFilePath(res.path);
      setDirty(false);
      return true;
    } catch (err) {
      setError(String(err));
      return false;
    }
  }, [title, directory]);

  // Save in place (Mod+S): the backend writes its exact PathBuf, so a
  // non-UTF-8 filename is safe (M9). Never uses the title.
  const saveInPlace = useCallback(async () => {
    try {
      const res = await api.saveDocument(docRef.current.markdown);
      if (api.isCanceled(res)) return false;
      setFilePath(res.path);
      setDirty(false);
      return true;
    } catch (err) {
      setError(String(err));
      return false;
    }
  }, []);

  const saveCurrent = useCallback(
    () => (docRef.current.documentPath ? saveInPlace() : saveAsDialog()),
    [saveInPlace, saveAsDialog]
  );
  saveCurrentRef.current = saveCurrent;

  const handleSaveAs = useCallback(() => saveAsDialog(), [saveAsDialog]);
  const handleSave = useCallback(
    () => (filePath ? saveInPlace() : saveAsDialog()),
    [filePath, saveInPlace, saveAsDialog]
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

  // The UI appearance preference (§8.6): persisted like any other setting,
  // but it never touches the render options or the document/page — it only
  // switches the Mantine color scheme (light / system / dark).
  const handleThemeChange = useCallback(
    (nextTheme) => {
      if (!nextTheme) return; // Mantine Select deselect guard
      const next = { ...settings, theme: nextTheme };
      setSettings(next);
      saveSettings(next, window.localStorage);
    },
    [settings]
  );

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
        setDirty(true);
        // Reflect the written settings immediately: the catch-up render is
        // deferred while the popover is open and may fail on unrelated
        // content, which must not leave the dialog showing stale values.
        setDocMeta((prev) => applyDocSettingsPatch(prev, patch));
        requestPreview();
      }),
    [runFileAction, markdown, requestPreview]
  );

  const handleExport = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.exportPdf(
          markdown,
          options,
          safeFileName(title),
          directory,
          filePath
        );
        if (api.isCanceled(res)) return;
        // The PDF is written and opened in the system viewer by the backend.
      }),
    [runFileAction, markdown, options, title, directory]
  );

  const dismissError = useCallback(() => setError(null), []);

  const focusEditor = useCallback(() => {
    setEditorFocusToken((token) => token + 1);
  }, []);

  // The command context is the registry's only view of React state/actions.
  const commandContext = useMemo(
    () => ({
      error,
      fileNew: handleNew,
      fileOpen: handleOpen,
      fileSave: handleSave,
      fileSaveAs: handleSaveAs,
      fileExport: handleExport,
      openDocumentSettings: () => handleSettingsOpenChange(true),
      openPageSettings: () => setPageSettingsOpen(true),
      openThemeMenu: () => setThemeMenuOpen(true),
      dismissError,
      focusEditor,
    }),
    [
      error,
      handleNew,
      handleOpen,
      handleSave,
      handleSaveAs,
      handleExport,
      handleSettingsOpenChange,
      dismissError,
      focusEditor,
    ]
  );

  /** One dispatcher for toolbar clicks, keyboard shortcuts and future menus. */
  const dispatch = useCallback(
    (commandId) => {
      const command = getCommand(commandId);
      if (!command || (command.canRun && !command.canRun(commandContext))) {
        return false;
      }
      try {
        const result = command.run(commandContext);
        if (result && typeof result.catch === "function") {
          result.catch((err) => setError(String(err)));
        }
      } catch (err) {
        setError(String(err));
      }
      return true;
    },
    [commandContext]
  );

  // Window-level capture-phase handling so application shortcuts work from
  // popovers, the menu and other non-editor focus targets. The latest
  // dispatcher/context is read through refs so this listener is attached once
  // per platform rather than on every keystroke.
  const dispatchRef = useRef(dispatch);
  const contextRef = useRef(commandContext);
  dispatchRef.current = dispatch;
  contextRef.current = commandContext;
  useEffect(() => {
    const onKeyDown = (event) => {
      handleKeydown(event, {
        platform,
        context: contextRef.current,
        dispatch: dispatchRef.current,
      });
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [platform]);

  // Preview links are intercepted in Preview.jsx and forwarded here: http(s)
  // opens in the system browser, anything else is rejected by the backend
  // with an error (§7.4 / §9) — the webview never navigates to a link.
  const handleExternalLink = useCallback(
    (url) => runFileAction(() => api.openExternal(url)),
    [runFileAction]
  );

  return (
    <MantineProvider theme={mantineTheme} forceColorScheme={colorScheme}>
      <div className={modalOpen ? "app modal-open" : "app"}>
        <Toolbar
          platform={platform}
          dispatch={dispatch}
          options={options}
          templates={templates}
          theme={theme}
          onThemeChange={handleThemeChange}
          onOptionChange={changeOption}
          onPickTemplate={handlePickTemplate}
          onClearTemplate={handleClearTemplate}
          docMeta={docMeta}
          onApplySettings={handleApplySettings}
          documentSettingsOpen={settingsOpen}
          onDocumentSettingsOpenChange={handleSettingsOpenChange}
          pageSettingsOpen={pageSettingsOpen}
          onPageSettingsOpenChange={setPageSettingsOpen}
          themeMenuOpen={themeMenuOpen}
          onThemeMenuOpenChange={setThemeMenuOpen}
        />
        <main id="panes" inert={modalOpen}>
          <section id="editor-pane">
            <Editor
              value={markdown}
              onChange={handleEditorChange}
              fontSize={editorFontSize}
              dark={colorScheme === "dark"}
              focusToken={editorFocusToken}
            />
          </section>
          <section id="preview-pane" data-rendering={renderBusy ? "true" : undefined}>
            <Preview pages={pages} busy={renderBusy} onExternalLink={handleExternalLink} />
          </section>
        </main>
        <StatusBar
          pageCount={pages.length}
          warnings={warnings}
          inert={modalOpen}
        />
        <ErrorBanner message={error} onClose={dismissError} />
        <UnsavedChangesModal
          open={confirmOpen}
          busy={confirmBusy}
          onChoose={chooseUnsaved}
        />
      </div>
    </MantineProvider>
  );
}
