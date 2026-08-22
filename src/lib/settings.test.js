// localStorage-backed settings (§8.6 of SPEC.md) — pure module, testable in
// vitest via an injected storage object (node has no `window.localStorage`).
import { describe, expect, it } from "vitest";
import {
  applyOptionsPatch,
  DEFAULT_SETTINGS,
  loadSettings,
  saveSettings,
} from "./settings.js";

/** A minimal in-memory stand-in for `window.localStorage`. */
function fakeStorage(initial = {}) {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => void map.set(k, String(v)),
  };
}

describe("loadSettings", () => {
  it("returns the defaults when nothing is stored", () => {
    const settings = loadSettings(fakeStorage());
    expect(settings).toEqual(DEFAULT_SETTINGS);
  });

  it("round-trips: settings saved for one session load on the next (M5)", () => {
    const storage = fakeStorage();
    const next = {
      options: {
        flavor: "custom",
        paper: "letter",
        marginPreset: "narrow",
        margins: { topMm: 15, bottomMm: 15, leftMm: 10, rightMm: 10 },
        textAlign: "justified",
        templatePath: null,
      },
      editorFontSize: 18,
    };
    saveSettings(next, storage);
    expect(loadSettings(storage)).toEqual(next);
  });

  it("falls back to defaults on corrupt JSON (M5)", () => {
    const storage = fakeStorage({ "tauri-md-editor.settings": "{not json" });
    expect(loadSettings(storage)).toEqual(DEFAULT_SETTINGS);
  });

  it("merges a partial stored options object onto the defaults", () => {
    const storage = fakeStorage({
      "tauri-md-editor.settings": JSON.stringify({
        options: { paper: "a5" },
      }),
    });
    const settings = loadSettings(storage);
    expect(settings.options.paper).toBe("a5");
    expect(settings.options.flavor).toBe(DEFAULT_SETTINGS.options.flavor);
    expect(settings.editorFontSize).toBe(DEFAULT_SETTINGS.editorFontSize);
  });

  it("falls back to default margins when stored margins are incomplete", () => {
    const storage = fakeStorage({
      "tauri-md-editor.settings": JSON.stringify({
        options: { margins: { topMm: 10 } },
      }),
    });
    expect(loadSettings(storage).options.margins).toEqual(
      DEFAULT_SETTINGS.options.margins
    );
  });
});

describe("saveSettings", () => {
  it("writes JSON under the tauri-md-editor.settings key", () => {
    const storage = fakeStorage();
    saveSettings(DEFAULT_SETTINGS, storage);
    const raw = storage.getItem("tauri-md-editor.settings");
    expect(JSON.parse(raw)).toEqual(DEFAULT_SETTINGS);
  });
});

describe("applyOptionsPatch", () => {
  it("ignores a null patch for a field that cannot be empty (Select deselect, §13)", () => {
    // Mantine Select fires onChange(null) when the already-selected option is
    // clicked again; applying that blindly corrupts the IPC payload.
    const next = applyOptionsPatch(DEFAULT_SETTINGS.options, { flavor: null });
    expect(next.flavor).toBe("custom");
  });

  it("keeps a null templatePath — clearing the custom template is legitimate (§8.4)", () => {
    const options = { ...DEFAULT_SETTINGS.options, templatePath: "/tmp/dark.typ" };
    const next = applyOptionsPatch(options, { templatePath: null });
    expect(next.templatePath).toBeNull();
  });

  it("applies real values and leaves unpatched fields untouched", () => {
    const next = applyOptionsPatch(DEFAULT_SETTINGS.options, {
      flavor: null, // deselect noise — dropped
      paper: "a5",
    });
    expect(next.paper).toBe("a5");
    expect(next.flavor).toBe("custom");
    expect(next.textAlign).toBe("left");
    expect(next.margins).toEqual(DEFAULT_SETTINGS.options.margins);
  });
});
