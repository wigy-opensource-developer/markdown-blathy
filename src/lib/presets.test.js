import { describe, expect, it } from "vitest";
import {
  ALIGNS,
  FLAVORS,
  MARGIN_PRESETS,
  PAPERS,
  marginsForPreset,
  renderOptionsPayload,
} from "./presets.js";

describe("FLAVORS", () => {
  it("uses the snake_case values the Rust Flavor enum expects (§6.1)", () => {
    expect(FLAVORS.map((f) => f.value)).toEqual(["commonmark", "gfm", "custom"]);
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
    flavor: "gfm",
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
