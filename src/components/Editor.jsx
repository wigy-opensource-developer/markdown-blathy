// CodeMirror 6 markdown editor via @uiw/react-codemirror (§8.2 of SPEC.md).
//
// `extensions` is memoized so CodeMirror is not reconfigured on every
// keystroke render. The component itself is memoized so the editor is left
// untouched when unrelated state (pages/warnings/error) changes.
//
// Application shortcuts (Save, Open, …) live in the window-level command
// registry, not here: they must work from popovers and other non-editor focus
// targets too. CodeMirror keeps only its editing keymap.
import { memo, useCallback, useEffect, useMemo, useRef } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

function Editor({
  value,
  onChange,
  fontSize = 16,
  dark = false,
  focusToken = 0,
  onView,
  onSyncScroll,
}) {
  const editorRef = useRef(null);
  const detachScrollRef = useRef(null);
  const extensions = useMemo(
    () => [
      history(),
      markdown(),
      keymap.of([...defaultKeymap, ...historyKeymap]),
    ],
    []
  );

  // New/Open dispatch through the command registry and bump this token; the
  // editor is the natural focus target after the document changes.
  useEffect(() => {
    if (focusToken > 0) editorRef.current?.view?.focus();
  }, [focusToken]);

  // Scroll sync uses CodeMirror's own line-block geometry. The wrapper
  // creates the EditorView in a layout effect and stores it in React state,
  // so the forwarded ref's `view` is still undefined when a mount effect
  // runs — onCreateEditor is the reliable hook, called synchronously with
  // the view at creation. The instance survives prop reconfigures; only a
  // remount (document load changes the key) creates a new one.
  //
  // Focus invariant: sync only ever writes scrollTop (here and in the
  // controller) — it never calls focus()/scrollIntoView(), so typing focus
  // stays wherever the user put it.
  const handleCreateEditor = useCallback(
    (view) => {
      onView?.(view);
      const scroller = view.scrollDOM;
      const scroll = () => onSyncScroll?.();
      scroller.addEventListener("scroll", scroll, { passive: true });
      detachScrollRef.current = () =>
        scroller.removeEventListener("scroll", scroll);
    },
    [onView, onSyncScroll]
  );

  // Unmount: detach the listener only. Do NOT clear the parent's view ref
  // here — on a keyed remount this passive cleanup runs AFTER the new
  // editor's onCreateEditor and would wipe the fresh view. The new mount
  // overwrites the ref; on a true unmount the controller dies with the app.
  useEffect(() => () => detachScrollRef.current?.(), []);

  return (
    <CodeMirror
      ref={editorRef}
      value={value}
      onChange={onChange}
      className="editor"
      height="100%"
      style={{ "--editor-font-size": `${fontSize}px` }}
      extensions={extensions}
      onCreateEditor={handleCreateEditor}
      // Disable autocompletion *and* its keymap: the default completion
      // keymap binds Alt-` on macOS. On layouts where ` is typed with
      // Option (e.g. Hungarian), that binding consumes the character and
      // prevents it from ever reaching the document. Markdown has no
      // completion source here, so the extension has nothing to offer.
      basicSetup={{
        foldGutter: false,
        autocompletion: false,
        completionKeymap: false,
      }}
      theme={dark ? "dark" : "light"}
    />
  );
}

export default memo(Editor);
