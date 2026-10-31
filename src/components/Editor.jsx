// CodeMirror 6 markdown editor via @uiw/react-codemirror (§8.2 of SPEC.md).
//
// `extensions` is memoized so CodeMirror is not reconfigured on every
// keystroke render. The component itself is memoized so the editor is left
// untouched when unrelated state (pages/warnings/error) changes.
import { memo, useMemo } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";

function Editor({ value, onChange, onSave, fontSize = 16, dark = false }) {
  const extensions = useMemo(
    () => [
      history(),
      markdown(),
      keymap.of([
        ...defaultKeymap,
        ...historyKeymap,
        { key: "Mod-s", run: () => { onSave(); return true; } },
      ]),
    ],
    [onSave]
  );

  return (
    <CodeMirror
      value={value}
      onChange={onChange}
      className="editor"
      height="100%"
      style={{ "--editor-font-size": `${fontSize}px` }}
      extensions={extensions}
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
