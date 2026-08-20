import { describe, expect, it } from "vitest";
import {
  ALIGNS,
  FEATURES,
  FEATURE_PRESETS,
  FLAVORS,
  MARGIN_PRESETS,
  PAPERS,
  marginsForPreset,
  renderOptionsPayload,
} from "./presets.js";

describe("FLAVORS", () => {
  it("uses the snake_case values the Rust Flavor enum expects (§6.1)", () => {
    expect(FLAVORS.map((f) => f.value)).toEqual(["commonmark", "github", "custom"]);
  });
});

describe("FEATURES / FEATURE_PRESETS", () => {
  it("lists every toggleable extension with a FeatureFlags key", () => {
    expect(FEATURES.map((f) => f.key)).toEqual([
      "strikethrough",
      "table",
      "task_list",
      "footnotes",
      "math",
      "highlight",
      "superscript",
      "subscript",
      "definition_list",
      "page_break",
    ]);
  });

  it("presets cover exactly the feature keys", () => {
    const keys = FEATURES.map((f) => f.key).sort();
    for (const name of ["commonmark", "github", "custom"]) {
      expect(Object.keys(FEATURE_PRESETS[name]).sort()).toEqual(keys);
    }
  });

  it("matches the Rust flavor preset table (§6.1)", () => {
    const cm = FEATURE_PRESETS.commonmark;
    expect(Object.values(cm).every((v) => v === false)).toBe(true);

    const gh = FEATURE_PRESETS.github;
    expect(gh.strikethrough && gh.table && gh.task_list).toBe(true);
    expect(gh.footnotes || gh.math || gh.highlight).toBe(false);

    const custom = FEATURE_PRESETS.custom;
    expect(Object.values(custom).every((v) => v === true)).toBe(true);
  });
});

describe("PAPERS", () => {
  it("uses the snake_case values the Rust Paper enum expects", () => {
    expect(PAPERS.map((p) => p.value)).toEqual(["a4", "a5", "letter", "legal"]);
  });
});

describe("ALIGNS", () => {
  it("uses the snake_case values the Rust TextAlign enum expects", () => {
    expect(ALIGNS.map((a) => a.value)).toEqual(["left", "center", "justified"]);
  });
});

describe("MARGIN_PRESETS", () => {
  it("exposes normal, narrow and custom presets", () => {
    expect(MARGIN_PRESETS.map((m) => m.value)).toEqual(["normal", "narrow", "custom"]);
  });
});

describe("marginsForPreset", () => {
  it("maps normal to 20/25mm (x=20, y=25 per §7.4)", () => {
    expect(marginsForPreset("normal")).toEqual({
      topMm: 25,
      bottomMm: 25,
      leftMm: 20,
      rightMm: 20,
    });
  });

  it("maps narrow to 10/15mm", () => {
    expect(marginsForPreset("narrow")).toEqual({
      topMm: 15,
      bottomMm: 15,
      leftMm: 10,
      rightMm: 10,
    });
  });

  it("returns null for custom (user-edited margins are a later milestone)", () => {
    expect(marginsForPreset("custom")).toBeNull();
  });
});

describe("renderOptionsPayload", () => {
  const options = {
    flavor: "github",
    paper: "a4",
    marginPreset: "normal",
    margins: { topMm: 25, bottomMm: 25, leftMm: 20, rightMm: 20 },
    textAlign: "left",
    templatePath: null,
  };

  it("drops the frontend-only marginPreset key", () => {
    expect(renderOptionsPayload(options)).not.toHaveProperty("marginPreset");
  });

  it("keeps the keys the Rust RenderOptions deserializes", () => {
    expect(Object.keys(renderOptionsPayload(options)).sort()).toEqual([
      "flavor",
      "margins",
      "paper",
      "templatePath",
      "textAlign",
    ]);
  });

  it("normalizes a missing templatePath to null", () => {
    const { templatePath } = options;
    const { marginPreset, ...rest } = options;
    expect(renderOptionsPayload(rest)).toEqual({
      ...rest,
      templatePath,
    });
  });
});
