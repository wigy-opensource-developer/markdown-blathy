// Window-level keyboard command dispatch (§9.2 of SPEC.md).
//
// Kept pure so the composition/AltGraph guards and the Escape fallback can be
// tested without a browser. App attaches this at the window capture phase.

import { commandForEvent, getCommand } from "./commands.js";

function mustIgnore(event) {
  return Boolean(
    event.isComposing ||
      event.keyCode === 229 ||
      event.altGraphKey ||
      event.getModifierState?.("AltGraph")
  );
}

/**
 * Dispatch the command matching `event`, if any.
 *
 * Returns true when a command was dispatched. Application shortcuts
 * preventDefault/stopPropagation so CodeMirror does not also handle them;
 * editing keys never reach this point because they do not match a command.
 */
export function handleKeydown(event, { platform, context, dispatch }) {
  if (event.repeat || mustIgnore(event)) return false;

  const commandId = commandForEvent(event, platform);
  if (commandId) {
    const command = getCommand(commandId);
    if (command?.canRun && !command.canRun(context)) return false;
    event.preventDefault?.();
    event.stopPropagation?.();
    dispatch(commandId);
    return true;
  }

  // Escape is the fallback for dismissing the error banner. Popovers keep
  // their own Escape handling when no error is visible.
  if (event.key === "Escape" && context.error) {
    event.preventDefault?.();
    event.stopPropagation?.();
    dispatch("error.dismiss");
    return true;
  }
  return false;
}
