// Platform detection for the command registry's keybindings.
//
// The GUI only needs to distinguish macOS's Cmd from Ctrl elsewhere. The input
// is injectable so the pure logic stays testable in node (no `navigator`).

export const PLATFORM_MAC = "mac";
export const PLATFORM_WINDOWS = "windows";
export const PLATFORM_LINUX = "linux";

/** Detect `mac` / `windows` / `linux` from a navigator-like object. */
export function detectPlatform(navigatorLike = globalThis.navigator) {
  const hints = [
    navigatorLike?.userAgentData?.platform,
    navigatorLike?.platform,
    navigatorLike?.userAgent,
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  if (hints.includes("mac") || hints.includes("iphone") || hints.includes("ipad")) {
    return PLATFORM_MAC;
  }
  if (hints.includes("win")) return PLATFORM_WINDOWS;
  return PLATFORM_LINUX;
}
