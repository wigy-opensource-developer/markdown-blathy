// Application command registry (§9.1 of SPEC.md).
//
// One place knows command ids, labels, default/platform keybindings, whether a
// command is currently available, and how to invoke the existing api.js
// wrappers or React state actions. Toolbar buttons, keyboard handlers and any
// future native menu all dispatch the same ids.
//
// The registry is pure: `run(ctx)` calls functions on the context supplied by
// App.jsx, so key resolution and dispatch are unit-testable without a browser.

import { detectPlatform, PLATFORM_MAC } from "./platform.js";

const modKeys = (key) => ({ mac: `Mod-${key}`, other: `Mod-${key}` });

export const COMMANDS = [
  {
    id: "file.new",
    label: "New document",
    keys: modKeys("N"),
    canRun: () => true,
    run: (ctx) => {
      ctx.fileNew();
      ctx.focusEditor();
    },
  },
  {
    id: "file.open",
    label: "Open Markdown file",
    keys: modKeys("O"),
    canRun: () => true,
    run: async (ctx) => {
      await ctx.fileOpen();
      ctx.focusEditor();
    },
  },
  {
    id: "file.save",
    label: "Save Markdown As…",
    keys: modKeys("S"),
    canRun: () => true,
    run: (ctx) => ctx.fileSave(),
  },
  {
    id: "file.exportPdf",
    label: "Export PDF",
    keys: modKeys("E"),
    canRun: () => true,
    run: (ctx) => ctx.fileExport(),
  },
  {
    id: "document.settings",
    label: "Document settings",
    keys: modKeys(","),
    canRun: () => true,
    run: (ctx) => ctx.openDocumentSettings(),
  },
  {
    id: "page.settings",
    label: "Page settings",
    keys: modKeys("Shift-,"),
    canRun: () => true,
    run: (ctx) => ctx.openPageSettings(),
  },
  {
    id: "theme.menu",
    label: "Color theme",
    keys: { mac: null, other: null },
    canRun: () => true,
    run: (ctx) => ctx.openThemeMenu(),
  },
  {
    id: "error.dismiss",
    label: "Dismiss error",
    keys: { mac: null, other: null },
    canRun: (ctx) => Boolean(ctx.error),
    run: (ctx) => ctx.dismissError(),
  },
];

const BY_ID = new Map(COMMANDS.map((command) => [command.id, command]));

/** Look up a command by id. */
export function getCommand(id) {
  return BY_ID.get(id) ?? null;
}

/** The raw key spec for the command on this platform, or null. */
export function commandKeySpec(commandOrId, platform = detectPlatform()) {
  const command = typeof commandOrId === "string" ? getCommand(commandOrId) : commandOrId;
  if (!command?.keys) return null;
  return platform === PLATFORM_MAC ? command.keys.mac : command.keys.other;
}

function normalizeKeyToken(token) {
  return token.length === 1 ? token.toLowerCase() : token.toLowerCase();
}

function parseKeySpec(spec) {
  const parts = { mod: false, alt: false, shift: false, key: null };
  for (const token of spec.split("-")) {
    const lower = token.toLowerCase();
    if (["mod", "cmd", "command", "ctrl", "control", "meta"].includes(lower)) {
      parts.mod = true;
    } else if (["alt", "option", "opt"].includes(lower)) {
      parts.alt = true;
    } else if (lower === "shift") {
      parts.shift = true;
    } else if (token.length > 0) {
      parts.key = normalizeKeyToken(token);
    }
  }
  return parts;
}

function canonicalChord(parts) {
  if (!parts.key) return null;
  return [
    parts.mod ? "Mod" : null,
    parts.alt ? "Alt" : null,
    parts.shift ? "Shift" : null,
    parts.key,
  ]
    .filter(Boolean)
    .join("+");
}

/** The canonical chord for a command on this platform. */
export function resolveKey(commandOrId, platform = detectPlatform()) {
  const spec = commandKeySpec(commandOrId, platform);
  return spec ? canonicalChord(parseKeySpec(spec)) : null;
}

const CODE_KEYS = {
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Minus: "-",
  Equal: "=",
  Backquote: "`",
};

function eventKeyToken(event) {
  const code = event.code ?? "";
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  if (CODE_KEYS[code]) return CODE_KEYS[code];
  return normalizeKeyToken(event.key ?? "");
}

/** The canonical chord of a keyboard event, or null when it must be ignored. */
export function eventChord(event, platform = detectPlatform()) {
  if (
    event.isComposing ||
    event.keyCode === 229 ||
    event.altGraphKey ||
    event.getModifierState?.("AltGraph")
  ) {
    return null;
  }
  const parts = {
    mod: platform === PLATFORM_MAC ? Boolean(event.metaKey) : Boolean(event.ctrlKey),
    alt: Boolean(event.altKey),
    shift: Boolean(event.shiftKey),
    key: eventKeyToken(event),
  };
  return canonicalChord(parts);
}

/** The command id for a keyboard event, or null when no command matches. */
export function commandForEvent(
  event,
  platform = detectPlatform(),
  commands = COMMANDS
) {
  const chord = eventChord(event, platform);
  if (!chord) return null;
  for (const command of commands) {
    if (resolveKey(command, platform) === chord) return command.id;
  }
  return null;
}

function displayKey(key) {
  if (key.length === 1) return key.toUpperCase();
  return key.charAt(0).toUpperCase() + key.slice(1);
}

/** Human-readable shortcut for tooltips, e.g. `Ctrl+S` / `Cmd+Shift+,`. */
export function shortcutLabel(commandOrId, platform = detectPlatform()) {
  const spec = commandKeySpec(commandOrId, platform);
  if (!spec) return "";
  const parts = parseKeySpec(spec);
  return [
    parts.mod ? (platform === PLATFORM_MAC ? "Cmd" : "Ctrl") : null,
    parts.alt ? "Alt" : null,
    parts.shift ? "Shift" : null,
    displayKey(parts.key),
  ]
    .filter(Boolean)
    .join("+");
}

function ariaKey(key) {
  if (key.length === 1) return key.toUpperCase();
  return key.charAt(0).toUpperCase() + key.slice(1);
}

/** The `aria-keyshortcuts` value for a command, or undefined. */
export function ariaKeyShortcut(commandOrId, platform = detectPlatform()) {
  const spec = commandKeySpec(commandOrId, platform);
  if (!spec) return undefined;
  const parts = parseKeySpec(spec);
  return [
    parts.mod ? (platform === PLATFORM_MAC ? "Meta" : "Control") : null,
    parts.alt ? "Alt" : null,
    parts.shift ? "Shift" : null,
    ariaKey(parts.key),
  ]
    .filter(Boolean)
    .join("+");
}

/** Tooltip text: label plus the platform shortcut when one is defined. */
export function commandTooltip(commandOrId, platform = detectPlatform()) {
  const command = typeof commandOrId === "string" ? getCommand(commandOrId) : commandOrId;
  if (!command) return "";
  const shortcut = shortcutLabel(command, platform);
  return shortcut ? `${command.label} (${shortcut})` : command.label;
}
