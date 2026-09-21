// Document settings form ↔ front-matter patch mapping (§6.2).
//
// The Document popover keeps a small form state and turns it into the patch
// `updateFrontMatter` applies. The tricky part is the tri-state IPC contract:
// an absent `hyphenate` key means "keep", JSON `null` means "clear" (back to
// auto) and a boolean sets it — `false` must travel as `false`, never as
// "missing". `lang` clears with an empty string, like title/author.
import { describe, expect, it } from "vitest";
import {
  buildDocSettingsPatch,
  hyphenateFormValue,
  hyphenatePatchValue,
} from "./docSettings.js";

/** The form shape the Document popover holds while it is open. */
const form = (overrides = {}) => ({
  title: "",
  author: "",
  autoHeadingLevel: 6,
  lang: "",
  hyphenate: "auto",
  features: { a: true },
  ...overrides,
});

describe("hyphenateFormValue", () => {
  it("maps the doc-meta value (null = auto) to the Select value", () => {
    expect(hyphenateFormValue(null)).toBe("auto");
    expect(hyphenateFormValue(undefined)).toBe("auto");
    expect(hyphenateFormValue(true)).toBe("true");
    expect(hyphenateFormValue(false)).toBe("false");
  });
});

describe("hyphenatePatchValue", () => {
  it("maps auto to null (clear) and the toggles to booleans", () => {
    expect(hyphenatePatchValue("auto")).toBeNull();
    expect(hyphenatePatchValue("true")).toBe(true);
    expect(hyphenatePatchValue("false")).toBe(false);
  });
});

describe("buildDocSettingsPatch", () => {
  it("returns an empty patch when nothing changed", () => {
    const f = form();
    expect(buildDocSettingsPatch(f, f)).toEqual({});
  });

  it("sends a changed language and clears it with an empty string", () => {
    const initial = form({ lang: "hu" });
    expect(buildDocSettingsPatch(form({ lang: "de" }), initial)).toEqual({
      lang: "de",
    });
    expect(buildDocSettingsPatch(form({ lang: "" }), initial)).toEqual({
      lang: "",
    });
  });

  it("sends hyphenate as a boolean and auto as an explicit null", () => {
    expect(buildDocSettingsPatch(form({ hyphenate: "false" }), form())).toEqual({
      hyphenate: false,
    });
    expect(buildDocSettingsPatch(form({ hyphenate: "true" }), form())).toEqual({
      hyphenate: true,
    });
    const off = form({ hyphenate: "false" });
    expect(buildDocSettingsPatch(form(), off)).toEqual({ hyphenate: null });
  });

  it("compares features regardless of key order", () => {
    const initial = form({ features: { tables: true, footnotes: false } });
    const same = form({ features: { footnotes: false, tables: true } });
    expect(buildDocSettingsPatch(same, initial)).toEqual({});
    const changed = form({ features: { footnotes: true, tables: true } });
    expect(buildDocSettingsPatch(changed, initial)).toEqual({
      features: { footnotes: true, tables: true },
    });
  });

  it("treats a missing features map as empty", () => {
    const initial = form({ features: undefined });
    expect(buildDocSettingsPatch(form({ features: {} }), initial)).toEqual({});
  });

  it("leaves untouched fields out of the patch", () => {
    const initial = form({
      title: "T",
      author: "A",
      autoHeadingLevel: 3,
      features: { x: true },
    });
    expect(buildDocSettingsPatch({ ...initial, lang: "hu" }, initial)).toEqual({
      lang: "hu",
    });
  });

  it("still reports title/author/auto/features changes", () => {
    const initial = form();
    const patch = buildDocSettingsPatch(
      {
        ...initial,
        title: "New",
        author: "Ada",
        autoHeadingLevel: 2,
        features: { a: false },
      },
      initial
    );
    expect(patch).toEqual({
      title: "New",
      author: "Ada",
      autoHeadingLevel: 2,
      features: { a: false },
    });
  });
});
