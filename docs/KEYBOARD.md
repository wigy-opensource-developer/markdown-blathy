# Keyboard reference

Shortcuts apply to the focused MarkdownBláthy window only. The application
does not register global/system-wide hotkeys.

`Mod` is **Ctrl** on Windows/Linux and **Cmd** on macOS.

| Command | Windows / Linux | macOS |
|---|---|---|
| New document | `Ctrl+N` | `Cmd+N` |
| Open Markdown | `Ctrl+O` | `Cmd+O` |
| Save Markdown As… | `Ctrl+S` | `Cmd+S` |
| Export PDF | `Ctrl+E` | `Cmd+E` |
| Document settings | `Ctrl+,` | `Cmd+,` |
| Page settings | `Ctrl+Shift+,` | `Cmd+Shift+,` |
| Dismiss error banner | `Esc` (when visible) | `Esc` (when visible) |
| Color theme menu | toolbar only | toolbar only |

The same command ids drive the toolbar buttons, these shortcuts, and any future
native menu entries. Tooltips and `aria-keyshortcuts` come from the registry in
`src/lib/commands.js`.

## Design rules

- Application shortcuts use `Mod` only; no plain Option/Alt-letter or
  `Ctrl+Alt` bindings, so national layouts, dead keys and AltGr keep working.
- CodeMirror keeps its own editing keymap (undo/redo, selection, word
  movement). The registry handles application commands from any focus target,
  including popovers.
- Keys are matched by physical key code where possible (letters and common
  punctuation), so shortcuts still work on non-US layouts.
- Events during IME composition, `keyCode === 229`, and AltGraph are ignored.
- After New/Open the editor takes focus. Mantine restores focus to the trigger
  when a popover/menu closes.
