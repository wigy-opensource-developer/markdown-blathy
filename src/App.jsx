import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { MantineProvider } from "@mantine/core";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import * as api from "./lib/api.js";
import { marginsForPreset } from "./lib/presets.js";
import { createRenderScheduler } from "./lib/renderScheduler.js";
import { computeLayout, visibleRange } from "./lib/previewWindow.js";
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
import { createUnsavedGuard, CHOICE_DISCARD } from "./lib/unsavedGuard.js";
import { createScrollSyncController } from "./lib/scrollSync.js";
import { AUTOSAVE_INTERVAL_MS, recoveryDecision } from "./lib/recovery.js";
import Toolbar from "./components/Toolbar.jsx";
import Editor from "./components/Editor.jsx";
import Preview from "./components/Preview.jsx";
import StatusBar from "./components/StatusBar.jsx";
import ErrorBanner from "./components/ErrorBanner.jsx";
import UnsavedChangesModal from "./components/UnsavedChangesModal.jsx";
import RecoveryPrompt from "./components/RecoveryPrompt.jsx";
import ExportWarnings from "./components/ExportWarnings.jsx";

export default function App() {
  // Settings are loaded once from localStorage (M5, §8.6) and are the single
  // source of truth for the render options; every option change persists
  // through `changeOption`. `marginPreset` is UI-only and stripped when the
  // payload is built (see lib/api.js); the rest mirrors RenderOptions (§6.1).
  const [settings, setSettings] = useState(() => loadSettings(window.localStorage));
  const { options, editorFontSize, theme, syncScroll } = settings;
  // The OS color scheme, tracked live so "system" follows changes at runtime.
  const [systemDark, setSystemDark] = useState(
    () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false
  );
  // The concrete Mantine color scheme for the UI chrome only.
  const colorScheme = resolveColorScheme(theme, systemDark);
  // The editor is UNCONTROLLED between loads: `markdown` is the text the
  // editor was *loaded* with (open/new/restore/front-matter edit), while the
  // live buffer lives in `docRef` and is updated on every keystroke without a
  // React commit. This is a performance requirement, not a style: a commit
  // per keystroke makes WebKitGTK run a full-document pass over the multi-MB
  // inline SVG preview, which takes longer than the 200 ms debounce — the
  // preview then re-renders once per character instead of once per pause.
  const [markdown, setMarkdown] = useState(SAMPLE_DOC);
  // Bumped on every load so the editor remounts with the new text even when
  // the new text equals the previous load (e.g. New after typing).
  const [documentVersion, setDocumentVersion] = useState(0);
  // True while the document has edits that are not on disk (M9): drives the
  // window-title marker and the unsaved-changes guard. A ref mirrors it for
  // the guard, whose callbacks outlive a render.
  const [dirty, setDirty] = useState(false);
  // The guard reads the ref, which flips on the first keystroke (the state
  // only follows at the debounced render); every save/load updates both.
  const dirtyRef = useRef(false);
  // The first keystroke marks the buffer dirty in a ref at once (the close
  // guard must never miss an edit) but defers the React state update to the
  // debounced render: a commit here would repaint the whole SVG preview
  // before the user has paused, reintroducing the per-character cost.
  const pendingDirtyRef = useRef(false);
  // Render metadata of the latest preview plus its initial window's SVGs
  // (seed): { seq, pageCount, pageSizes, pageBackground, seed }. The window
  // is exported with the compile so the swap from the last-good pages is
  // atomic; scrolling exports further pages on demand from the backend's
  // single cached document — no state here ever holds the whole multi-MB
  // preview. A ref mirrors it for the export callback, whose responses
  // outlive a render.
  const [preview, setPreview] = useState(null);
  const previewRef = useRef(null);
  const editorViewRef = useRef(null);
  const previewScrollerRef = useRef(null);
  // Source maps are valid only for the exact editor revision they compiled.
  // Geometry-only option renders may reuse the same revision; edits and loads
  // bump it immediately, suspending sync while the last-good preview is stale.
  const editorRevisionRef = useRef(0);
  const previewRevisionRef = useRef(-1);
  const renderRevisionRef = useRef(null);
  const scrollSyncRef = useRef(null);
  if (!scrollSyncRef.current) {
    scrollSyncRef.current = createScrollSyncController({
      getEditorView: () => editorViewRef.current,
      getPreviewScroller: () => previewScrollerRef.current,
      getPreview: () => previewRef.current,
      isPreviewCurrent: () => editorRevisionRef.current === previewRevisionRef.current,
    });
  }
  // The preview pane's scroll window, reported by Preview (no state — a
  // scroll must not commit React). runRender reads it to ask for the pages
  // the user is looking at, so a render swaps old → new atomically.
  const viewportRef = useRef({ width: 0, height: 0, top: 0 });
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
  // Latest preview sequence: bumped when a render is sent, and when a load
  // invalidates an in-flight response; older responses are dropped (§7.5).
  const seq = useRef(0);
  // The render scheduler, built below once the render callback it drives
  // exists (§6.2; see lib/renderScheduler.js).
  const scheduler = useRef(null);
  // The delayed "Rendering…" state machine (lib/renderStatus.js), built
  // below with its React binding.
  const renderStatus = useRef(null);
  // The document the *next render* will use. Normally synced from state on
  // every render — but a freshly opened file is written here before calling
  // `requestPreviewNow`, because the immediate render runs before React has
  // committed the new `markdown`. `documentPath` lets the backend resolve
  // document-relative images.
  const docRef = useRef({ markdown: SAMPLE_DOC, options, documentPath: null });
  // Only non-buffer fields are synced from render state; `markdown` is owned
  // by the editor and the load helper (a render must not clobber typed text).
  docRef.current.options = options;
  docRef.current.documentPath = filePath;

  // The effective document settings from the last render (§6.2): Rust parses
  // the front matter and returns it, so the frontend never parses YAML
  // itself. Applied settings patches are merged in immediately (the canonical
  // value Rust just wrote), so the dialog never shows stale values while the
  // catch-up render is pending. Null until the first render lands.
  const [docMeta, setDocMeta] = useState(null);

  // Modals (the Document settings popover and the unsaved-changes prompt)
  // share one treatment: the editor/preview/status are blurred, `inert`
  // (clicks, caret and screen readers stay out) and frozen (see
  // `.modal-open` in styles.css and lib/renderScheduler.js). The counter keeps
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
  // Post-export compile warnings (M9 task 5): `{ path, warnings }` shown in a
  // dismissible panel; null when the export was warning-free.
  const [exportWarnings, setExportWarnings] = useState(null);
  // Crash recovery (M9 task 4): the snapshot offered at startup, if any.
  const [recovery, setRecovery] = useState(null);
  const recoveryOpenRef = useRef(false);
  // True until the user changes anything: only a pristine editor may be
  // replaced by a recovery snapshot (otherwise the snapshot waits for the
  // next launch instead of eating their work).
  const pristineRef = useRef(true);
  // The startup recovery check must run at most once per mount (StrictMode
  // re-runs effects in development).
  const recoveryCheckedRef = useRef(false);
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

  /** A finished render lands as metadata PLUS the initial window's SVGs,
   *  exported from the same compile. One commit swaps the last-good pages
   *  for the new ones — no placeholder gap, and the pane's "Rendering…"
   *  note (armed only for slow renders) decorates the OLD pages until this
   *  moment, exactly as before virtualization. Scrolling exports further
   *  pages window-sized via export_pages. */
  const applyPreview = useCallback((result) => {
    const meta = {
      seq: result.seq,
      pageCount: result.pageCount,
      pageSizes: result.pageSizes,
      pageBackground: result.pageBackground ?? null,
      sourceAnchors: result.sourceAnchors ?? [],
      seed: result.pages ?? [],
    };
    const revision = renderRevisionRef.current;
    previewRevisionRef.current = revision?.seq === result.seq ? revision.revision : -1;
    previewRef.current = meta;
    setPreview(meta);
    setWarnings(result.warnings);
    setDocMeta(result.doc);
    setError(null);
    renderStatus.current.stop(); // the pending note comes down with the swap
  }, []);

  // Delayed "Rendering…" feedback on the preview pane: keystroke renders
  // usually land before the note even arms (350 ms), so it never flickers;
  // big documents do pend long enough to show it. lib/renderStatus.js owns
  // the timing; this binding just mirrors visibility into React state.
  const [renderBusy, setRenderBusy] = useState(false);
  if (!renderStatus.current) {
    renderStatus.current = createRenderStatus({ delayMs: 350, notify: setRenderBusy });
  }

  // The render body the scheduler starts: reads `docRef`, so it always
  // compiles the live buffer — a load in the same event included. Returns
  // the result for the scheduler to apply, or null when the response was
  // invalidated by a newer load (seq guard, §7.5) or the compile failed;
  // either way the scheduler's catch-up check still runs. All its captures
  // are refs/stable setters, so this callback is stable.
  const runRender = useCallback(async () => {
    if (pendingDirtyRef.current) {
      pendingDirtyRef.current = false;
      setDirty(true); // title dot + autosave start; the user has paused
    }
    renderStatus.current.start();
    const seqId = ++seq.current;
    renderRevisionRef.current = { seq: seqId, revision: editorRevisionRef.current };
    const { markdown: doc, options: opts, documentPath: docPath } = docRef.current;
    // Ask for the window the user is looking at, so the response can swap
    // old → new in one commit. Sizes come from the previous render (paper
    // rarely changes mid-edit; indices are clamped backend-side either way).
    // Before the first measurement this is empty: metadata only, and the
    // Preview fetches its window once measured.
    const vp = viewportRef.current;
    const layout = computeLayout(previewRef.current?.pageSizes ?? [], vp.width);
    const { first, last } = visibleRange(layout, vp.top, vp.height);
    const window = [];
    for (let i = first; i <= last; i++) window.push(i);
    try {
      const result = await api.renderPreview(doc, opts, seqId, docPath, window);
      // Invalidated by a load: drop the pages, keep the "Rendering…" note —
      // the load's render is the live episode now.
      if (isStaleResponse(result.seq, seq.current)) return null;
      return result;
    } catch (err) {
      renderStatus.current.stop();
      setError(String(err)); // keep the last good preview
      return null;
    }
  }, []);

  // The render cycle has ONE owner (lib/renderScheduler.js): edits are
  // debounced 200 ms (well within the ≤300 ms acceptance for M3); at most
  // one compile is in flight — edits meanwhile only mark a catch-up owed,
  // never start or schedule a second render; when the render settles, the
  // catch-up starts iff there were edits since it began and the last one is
  // ≥200 ms old (otherwise the remaining debounce is armed). The modal
  // freeze (§6.2) lives in the same machine: while a popover is open nothing
  // renders and finished results are stashed for one catch-up pass on close.
  // `invalidate` bumps the seq so a load drops an in-flight response even
  // though its own render waits for that one to settle.
  if (!scheduler.current) {
    scheduler.current = createRenderScheduler({
      debounceMs: 200,
      render: runRender,
      apply: applyPreview,
      invalidate: () => {
        seq.current += 1;
      },
    });
  }

  /** Load `text` as a fresh document: the editor remounts with it, the live
   *  buffer follows, and the next render reads the same text. */
  const loadDocument = useCallback((text) => {
    editorRevisionRef.current += 1;
    scrollSyncRef.current?.reset();
    docRef.current.markdown = text;
    setMarkdown(text);
    setDocumentVersion((version) => version + 1);
  }, []);

  /** An edit needs a preview: the scheduler debounces it, or remembers it
   *  as owed while a render is in flight / a modal is open. */
  const requestPreview = useCallback(() => scheduler.current.request(), []);

  /**
   * Render a just-loaded document right away (no 200 ms debounce). `patch`
   * carries the fresh text when the caller's load has not updated the live
   * buffer yet — the initial sample needs no patch. If a render is in
   * flight, its response is invalidated and the load renders as soon as it
   * settles (still one compile at a time).
   */
  const requestPreviewNow = useCallback((patch) => {
    if (patch) docRef.current = { ...docRef.current, ...patch };
    scheduler.current.requestNow();
  }, []);

  /** Enter a modal interaction: the first one freezes previews and blurs the
   *  panes; the scheduler catches up once when the last one closes. */
  const enterModal = useCallback(() => {
    if (modalDepthRef.current++ === 0) scheduler.current.open();
    setModalDepth(modalDepthRef.current);
  }, []);
  const leaveModal = useCallback(() => {
    if (modalDepthRef.current === 0) return; // unpaired notification
    if (--modalDepthRef.current === 0) scheduler.current.close();
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
   *  a Save runs (it may open the Save As dialog). Discard also drops the
   *  recovery snapshot: the edits are gone by the user's own choice. */
  const chooseUnsaved = useCallback(async (choice) => {
    setConfirmBusy(true);
    try {
      if (choice === CHOICE_DISCARD) {
        await api.clearRecovery().catch(() => {});
      }
      await guardRef.current.choose(choice);
    } finally {
      setConfirmBusy(false);
    }
  }, []);

  // The startup check and the close helper are referenced by callbacks that
  // are declared earlier (the request listener, Open/New); they reach the
  // latest values through these refs. Declared here, before the assignment
  // during render.
  const checkRecoveryRef = useRef(() => {});
  const closeRecoveryRef = useRef(() => {});

  /** Startup recovery check (M9 task 4). Runs at most once per mount and
   *  only *offers* the snapshot — nothing is applied without a choice.
   *  `expectedPath` comes from a CLI-opened document: only that document's
   *  own snapshot may be offered then. */
  const checkRecovery = useCallback(
    async (expectedPath = null) => {
      if (recoveryCheckedRef.current) return;
      recoveryCheckedRef.current = true;
      try {
        const snapshot = await api.takeRecovery();
        const decision = recoveryDecision({
          snapshot,
          pristine: pristineRef.current,
          expectedPath,
        });
        if (decision !== "offer") return;
        recoveryOpenRef.current = true;
        setRecovery(snapshot);
        enterModal();
      } catch {
        // Recovery is best-effort: an unreadable store just skips the prompt.
      }
    },
    [enterModal]
  );

  const closeRecovery = useCallback(() => {
    if (!recoveryOpenRef.current) return; // already closed
    recoveryOpenRef.current = false;
    setRecovery(null);
    leaveModal();
  }, [leaveModal]);

  checkRecoveryRef.current = checkRecovery;
  closeRecoveryRef.current = closeRecovery;

  const handleRestoreRecovery = useCallback(async () => {
    const snapshot = recovery;
    if (!snapshot) return;
    try {
      const res = await api.restoreRecovery(snapshot.id);
      setFilePath(res.path);
      loadDocument(res.markdown);
      dirtyRef.current = true;
      setDirty(true);
      pristineRef.current = false;
      if (res.path) setDirectory(parentDirectory(res.path));
      requestPreviewNow({ markdown: res.markdown, documentPath: res.path });
    } catch (err) {
      setError(String(err));
    } finally {
      closeRecovery();
    }
  }, [recovery, loadDocument, requestPreviewNow, closeRecovery]);

  const handleDiscardRecovery = useCallback(async () => {
    const snapshot = recovery;
    if (!snapshot) return;
    try {
      await api.discardRecovery(snapshot.id);
    } catch (err) {
      setError(String(err));
    } finally {
      closeRecovery();
    }
  }, [recovery, closeRecovery]);

  /** Popover opened → freeze; closed → the scheduler catches up once. */
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
    return () => {
      scheduler.current.dispose();
      scrollSyncRef.current?.dispose();
    };
  }, [requestPreviewNow]);

  // The toggle is UI-only: enabling aligns the last active pane immediately,
  // disabling never triggers a render.
  useEffect(() => {
    scrollSyncRef.current?.setEnabled(syncScroll);
  }, [syncScroll]);

  // Page sizes and anchors commit together. Re-align once after that commit
  // so the same source focal point survives layout changes and re-renders.
  useEffect(() => {
    scrollSyncRef.current?.resynchronize();
  }, [preview]);

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
        const pending = await api.frontendReady();
        // A CLI document opens through its own request; it runs the recovery
        // check on completion (and only for its own snapshot).
        if (!pending) checkRecoveryRef.current();
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
      // A programmatic load echoes through CodeMirror's onChange; it is not a
      // user edit (loadDocument already set the buffer).
      if (value === docRef.current.markdown) return;
      pristineRef.current = false;
      editorRevisionRef.current += 1;
      docRef.current.markdown = value;
      if (!dirtyRef.current) {
        dirtyRef.current = true;
        pendingDirtyRef.current = true;
      }
      requestPreview();
    },
    [requestPreview]
  );

  // Autosave (M9 task 4): while dirty, snapshot immediately and then every
  // 30 s. Failures are ignored — autosave must never interrupt editing; the
  // backend drops the snapshot after a successful save.
  useEffect(() => {
    if (!dirty) return;
    let cancelled = false;
    const snapshot = () => {
      if (cancelled) return;
      api.saveRecoverySnapshot(docRef.current.markdown).catch(() => {});
    };
    snapshot();
    const timer = setInterval(snapshot, AUTOSAVE_INTERVAL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [dirty]);

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
      // Keep the offered snapshot for the next launch; the document the user
      // asked for must not sit under a recovery prompt for another file.
      closeRecoveryRef.current();
      setFilePath(res.path);
      loadDocument(res.content);
      pendingDirtyRef.current = false;
      dirtyRef.current = false;
      setDirty(false);
      setDirectory(parentDirectory(res.path));
      requestPreviewNow({ markdown: res.content, documentPath: res.path });
    },
    [loadDocument, requestPreviewNow]
  );

  // New document: clear the backend's save-in-place path first. Awaiting it
  // means a failure keeps the old document (and its path) rather than leaving
  // the UI untitled while a plain save would still target the old file.
  const doNew = useCallback(
    () =>
      runFileAction(async () => {
        closeRecoveryRef.current();
        await api.forgetDocument();
        setFilePath(null);
        loadDocument(SAMPLE_DOC);
        pendingDirtyRef.current = false;
        dirtyRef.current = false;
        setDirty(false);
        requestPreviewNow({ markdown: SAMPLE_DOC, documentPath: null });
      }),
    [runFileAction, loadDocument, requestPreviewNow]
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
        checkRecoveryRef.current(res.path);
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
      pendingDirtyRef.current = false;
      dirtyRef.current = false;
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
      pendingDirtyRef.current = false;
      dirtyRef.current = false;
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
  const handleToggleSyncScroll = useCallback(() => {
    const next = { ...settings, syncScroll: !settings.syncScroll };
    setSettings(next);
    saveSettings(next, window.localStorage);
    // Clicking the toggle must not cost the user their keyboard focus:
    // hand it back to the editor. preventScroll keeps the viewport where it
    // is (no jump to the cursor). While a modal is open the panes are inert
    // and this is a silent no-op.
    editorViewRef.current?.contentDOM.focus({ preventScroll: true });
  }, [settings]);

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
        const updated = await api.updateFrontMatter(docRef.current.markdown, patch);
        loadDocument(updated);
        pristineRef.current = false;
        dirtyRef.current = true;
        setDirty(true);
        // Reflect the written settings immediately: the catch-up render is
        // deferred while the popover is open and may fail on unrelated
        // content, which must not leave the dialog showing stale values.
        setDocMeta((prev) => applyDocSettingsPatch(prev, patch));
        requestPreview();
      }),
    [runFileAction, loadDocument, requestPreview]
  );

  const handleExport = useCallback(
    () =>
      runFileAction(async () => {
        const res = await api.exportPdf(
          docRef.current.markdown,
          options,
          safeFileName(title),
          directory,
          filePath
        );
        if (api.isCanceled(res)) return;
        // The PDF is written and opened in the system viewer by the backend;
        // compile warnings get their own dismissible panel (M9 task 5).
        setExportWarnings(
          res.warnings?.length
            ? { path: res.path, warnings: res.warnings }
            : null
        );
      }),
    [runFileAction, options, title, directory, filePath]
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

  // Export-on-demand for the virtualized preview: SVG pages from the
  // backend's single cached render (no recompile). Reads the live seq ref;
  // a response superseded by a newer render is dropped. Rejects on failure
  // so Preview clears its in-flight marks (the banner shows why).
  const handleFetchPages = useCallback(async (indices) => {
    const seq = previewRef.current?.seq;
    if (seq == null) return [];
    try {
      const res = await api.exportPages(seq, indices);
      if (res.stale || previewRef.current?.seq !== seq) return [];
      return res.pages;
    } catch (err) {
      setError(String(err));
      throw err;
    }
  }, []);

  // Preview reports its scroll window here (a ref write, not a commit).
  const handleViewport = useCallback((vp) => {
    viewportRef.current = vp;
  }, []);
  const handleEditorView = useCallback((view) => {
    editorViewRef.current = view;
  }, []);
  const handlePreviewScroller = useCallback((scroller) => {
    previewScrollerRef.current = scroller;
  }, []);
  const handleEditorSyncScroll = useCallback(() => {
    scrollSyncRef.current?.onScroll("editor");
  }, []);
  const handlePreviewSyncScroll = useCallback(() => {
    scrollSyncRef.current?.onScroll("preview");
  }, []);

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
          syncScroll={syncScroll}
          onToggleSyncScroll={handleToggleSyncScroll}
        />
        <main id="panes" inert={modalOpen}>
          <section id="editor-pane">
            <Editor
              key={documentVersion}
              value={markdown}
              onChange={handleEditorChange}
              fontSize={editorFontSize}
              dark={colorScheme === "dark"}
              focusToken={editorFocusToken}
              onView={handleEditorView}
              onSyncScroll={handleEditorSyncScroll}
            />
          </section>
          <section id="preview-pane" data-rendering={renderBusy ? "true" : undefined}>
            <Preview
              preview={preview}
              fetchPages={handleFetchPages}
              onViewport={handleViewport}
              onScroller={handlePreviewScroller}
              onSyncScroll={handlePreviewSyncScroll}
              busy={renderBusy}
              onExternalLink={handleExternalLink}
            />
          </section>
        </main>
        <StatusBar
          pageCount={preview?.pageCount ?? 0}
          warnings={warnings}
          inert={modalOpen}
        />
        <ErrorBanner message={error} onClose={dismissError} />
        <ExportWarnings
          notice={exportWarnings}
          onClose={() => setExportWarnings(null)}
        />
        <UnsavedChangesModal
          open={confirmOpen}
          busy={confirmBusy}
          onChoose={chooseUnsaved}
        />
        <RecoveryPrompt
          snapshot={recovery}
          onRestore={handleRestoreRecovery}
          onDiscard={handleDiscardRecovery}
          onClose={closeRecovery}
        />
      </div>
    </MantineProvider>
  );
}
