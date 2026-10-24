import { describe, expect, it } from "vitest";
import { UI_THEMES, isUiTheme, resolveColorScheme } from "./uiTheme.js";

describe("UI_THEMES", () => {
  it("offers light, system and dark in that order", () => {
    expect(UI_THEMES.map((t) => t.value)).toEqual(["light", "system", "dark"]);
  });
});

describe("isUiTheme", () => {
  it("accepts exactly the three theme values", () => {
    expect(["light", "system", "dark"].every(isUiTheme)).toBe(true);
    expect(isUiTheme("sepia")).toBe(false);
    expect(isUiTheme(undefined)).toBe(false);
    expect(isUiTheme(null)).toBe(false);
  });
});

describe("resolveColorScheme", () => {
  it("maps light and dark to themselves regardless of the OS", () => {
    expect(resolveColorScheme("light", true)).toBe("light");
    expect(resolveColorScheme("light", false)).toBe("light");
    expect(resolveColorScheme("dark", true)).toBe("dark");
    expect(resolveColorScheme("dark", false)).toBe("dark");
  });

  it("follows the OS preference for system", () => {
    expect(resolveColorScheme("system", true)).toBe("dark");
    expect(resolveColorScheme("system", false)).toBe("light");
  });

  it("treats an unknown value as system", () => {
    expect(resolveColorScheme("sepia", true)).toBe("dark");
    expect(resolveColorScheme(undefined, false)).toBe("light");
  });
});
