import { describe, expect, it } from "vitest";
import { detectPlatform, PLATFORM_LINUX, PLATFORM_MAC, PLATFORM_WINDOWS } from "./platform.js";

describe("platform detection", () => {
  it("recognises macOS from platform or user agent", () => {
    expect(detectPlatform({ platform: "MacIntel" })).toBe(PLATFORM_MAC);
    expect(detectPlatform({ userAgent: "Mozilla/5.0 (Macintosh)" })).toBe(PLATFORM_MAC);
    expect(
      detectPlatform({ userAgentData: { platform: "macOS" } })
    ).toBe(PLATFORM_MAC);
  });

  it("recognises Windows", () => {
    expect(detectPlatform({ platform: "Win32" })).toBe(PLATFORM_WINDOWS);
    expect(detectPlatform({ userAgent: "Mozilla/5.0 (Windows NT 10.0)" })).toBe(
      PLATFORM_WINDOWS
    );
  });

  it("falls back to Linux", () => {
    expect(detectPlatform({ platform: "Linux x86_64" })).toBe(PLATFORM_LINUX);
    expect(detectPlatform(undefined)).toBe(PLATFORM_LINUX);
  });
});
