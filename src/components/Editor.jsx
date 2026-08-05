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

function Editor({ value, onChange, onSave, fontSize = 16 }) {
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
      basicSetup={{ foldGutter: false }}
    />
  );
}

export default memo(Editor);
