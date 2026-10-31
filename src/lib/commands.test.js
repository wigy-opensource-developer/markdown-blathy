import { describe, expect, it } from "vitest";
import {
  COMMANDS,
  ariaKeyShortcut,
  commandForEvent,
  commandTooltip,
  getCommand,
  resolveKey,
  shortcutLabel,
} from "./commands.js";
import { PLATFORM_LINUX, PLATFORM_MAC } from "./platform.js";

const keyEvent = (props) => ({
  key: "s",
  code: "KeyS",
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  isComposing: false,
  keyCode: 83,
  altGraphKey: false,
  getModifierState: () => false,
  ...props,
});

describe("command registry", () => {
  it("looks commands up by id", () => {
    expect(getCommand("file.save")?.label).toBe("Save Markdown As…");
    expect(getCommand("missing")).toBeNull();
  });

  it("resolves Mod to per-platform key chords", () => {
    expect(resolveKey("file.save", PLATFORM_MAC)).toBe("Mod+s");
    expect(resolveKey("file.save", PLATFORM_LINUX)).toBe("Mod+s");
    expect(resolveKey("document.settings", PLATFORM_LINUX)).toBe("Mod+,");
    expect(resolveKey("page.settings", PLATFORM_LINUX)).toBe("Mod+Shift+,");
    expect(resolveKey("theme.menu", PLATFORM_MAC)).toBeNull();
  });

  it("has no duplicate or ambiguous bindings", () => {
    for (const platform of [PLATFORM_MAC, PLATFORM_LINUX]) {
      const chords = COMMANDS.map((command) => resolveKey(command, platform)).filter(Boolean);
      expect(new Set(chords).size, `${platform}: ${chords.join(", ")}`).toBe(chords.length);
    }
  });

  it("does not use Alt-only bindings", () => {
    for (const command of COMMANDS) {
      for (const spec of [command.keys.mac, command.keys.other]) {
        if (!spec) continue;
        const tokens = spec.split("-").map((token) => token.toLowerCase());
        const hasAlt = tokens.includes("alt") || tokens.includes("option");
        const hasMod = tokens.some((token) =>
          ["mod", "cmd", "command", "ctrl", "control", "meta"].includes(token)
        );
        expect(hasAlt && !hasMod, `${command.id}: ${spec}`).toBe(false);
      }
    }
  });

  it("maps keyboard events to command ids", () => {
    expect(commandForEvent(keyEvent({ ctrlKey: true }), PLATFORM_LINUX)).toBe("file.save");
    expect(commandForEvent(keyEvent({ metaKey: true }), PLATFORM_MAC)).toBe("file.save");
    expect(
      commandForEvent(
        keyEvent({ ctrlKey: true, shiftKey: true, key: "<", code: "Comma" }),
        PLATFORM_LINUX
      )
    ).toBe("page.settings");
    expect(
      commandForEvent(
        keyEvent({ ctrlKey: true, key: ",", code: "Comma" }),
        PLATFORM_LINUX
      )
    ).toBe("document.settings");
  });

  it("does not fire while composing, during AltGraph, or for plain keys", () => {
    expect(commandForEvent(keyEvent({ ctrlKey: true, isComposing: true }), PLATFORM_LINUX)).toBeNull();
    expect(commandForEvent(keyEvent({ ctrlKey: true, keyCode: 229 }), PLATFORM_LINUX)).toBeNull();
    expect(
      commandForEvent(keyEvent({ ctrlKey: true, altGraphKey: true }), PLATFORM_LINUX)
    ).toBeNull();
    expect(
      commandForEvent(
        keyEvent({ ctrlKey: true, getModifierState: (key) => key === "AltGraph" }),
        PLATFORM_LINUX
      )
    ).toBeNull();
    expect(commandForEvent(keyEvent({}), PLATFORM_LINUX)).toBeNull();
    expect(commandForEvent(keyEvent({ altKey: true }), PLATFORM_LINUX)).toBeNull();
  });

  it("formats tooltips and ARIA shortcuts from the registry", () => {
    expect(shortcutLabel("file.save", PLATFORM_LINUX)).toBe("Ctrl+S");
    expect(shortcutLabel("file.save", PLATFORM_MAC)).toBe("Cmd+S");
    expect(shortcutLabel("page.settings", PLATFORM_LINUX)).toBe("Ctrl+Shift+,");
    expect(ariaKeyShortcut("file.save", PLATFORM_LINUX)).toBe("Control+S");
    expect(ariaKeyShortcut("file.save", PLATFORM_MAC)).toBe("Meta+S");
    expect(ariaKeyShortcut("theme.menu", PLATFORM_LINUX)).toBeUndefined();
    expect(commandTooltip("file.save", PLATFORM_LINUX)).toBe(
      "Save Markdown As… (Ctrl+S)"
    );
  });
});
