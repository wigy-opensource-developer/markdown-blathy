// localStorage-backed settings (§8.6 of SPEC.md) — pure module, testable in
// vitest via an injected storage object (node has no `window.localStorage`).
import { describe, expect, it } from "vitest";
import {
  applyOptionsPatch,
  DEFAULT_SETTINGS,
  loadSettings,
  pageSetupDefaultsPatch,
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
        templateName: null,
        templatePath: null,
      },
      editorFontSize: 18,
      theme: "dark",
    };
    saveSettings(next, storage);
    expect(loadSettings(storage)).toEqual(next);
  });

  it("falls back to defaults on corrupt JSON (M5)", () => {
    const storage = fakeStorage({ "markdown-blathy.settings": "{not json" });
    expect(loadSettings(storage)).toEqual(DEFAULT_SETTINGS);
  });

  it("merges a partial stored options object onto the defaults", () => {
    const storage = fakeStorage({
      "markdown-blathy.settings": JSON.stringify({
        options: { paper: "a5" },
      }),
    });
    const settings = loadSettings(storage);
    expect(settings.options.paper).toBe("a5");
    expect(settings.options.flavor).toBe(DEFAULT_SETTINGS.options.flavor);
    expect(settings.editorFontSize).toBe(DEFAULT_SETTINGS.editorFontSize);
    expect(settings.theme).toBe(DEFAULT_SETTINGS.theme);
  });

  it("falls back to the system theme for an unknown theme value", () => {
    const storage = fakeStorage({
      "markdown-blathy.settings": JSON.stringify({ theme: "sepia" }),
    });
    expect(loadSettings(storage).theme).toBe("system");
  });

  it("falls back to default margins when stored margins are incomplete", () => {
    const storage = fakeStorage({
      "markdown-blathy.settings": JSON.stringify({
        options: { margins: { topMm: 10 } },
      }),
    });
    expect(loadSettings(storage).options.margins).toEqual(
      DEFAULT_SETTINGS.options.margins
    );
  });
});

describe("saveSettings", () => {
  it("writes JSON under the markdown-blathy.settings key", () => {
    const storage = fakeStorage();
    saveSettings(DEFAULT_SETTINGS, storage);
    const raw = storage.getItem("markdown-blathy.settings");
    expect(JSON.parse(raw)).toEqual(DEFAULT_SETTINGS);
  });
});

describe("pageSetupDefaultsPatch", () => {
  it("restores paper, margins, alignment and template in one patch", () => {
    expect(pageSetupDefaultsPatch()).toEqual({
      paper: DEFAULT_SETTINGS.options.paper,
      marginPreset: DEFAULT_SETTINGS.options.marginPreset,
      textAlign: DEFAULT_SETTINGS.options.textAlign,
      templateName: DEFAULT_SETTINGS.options.templateName,
      templatePath: DEFAULT_SETTINGS.options.templatePath,
    });
  });

  it("clears a custom template — null is a real value (§8.4)", () => {
    expect(pageSetupDefaultsPatch().templatePath).toBeNull();
    expect(pageSetupDefaultsPatch().templateName).toBeNull();
  });
});

describe("applyOptionsPatch", () => {
  it("ignores a null patch for a field that cannot be empty (Select deselect, §13)", () => {
    // Mantine Select fires onChange(null) when the already-selected option is
    // clicked again; applying that blindly corrupts the IPC payload.
    const next = applyOptionsPatch(DEFAULT_SETTINGS.options, { flavor: null });
    expect(next.flavor).toBe("custom");
  });

  it("keeps a null templateName/templatePath — clearing the template is legitimate (§8.4)", () => {
    const options = {
      ...DEFAULT_SETTINGS.options,
      templateName: "dark",
      templatePath: "/tmp/dark.typ",
    };
    const next = applyOptionsPatch(options, {
      templateName: null,
      templatePath: null,
    });
    expect(next.templateName).toBeNull();
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
