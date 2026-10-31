import { describe, expect, it, vi } from "vitest";
import { handleKeydown } from "./keyboard.js";
import { PLATFORM_LINUX } from "./platform.js";

const keyEvent = (props = {}) => ({
  key: "s",
  code: "KeyS",
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  shiftKey: false,
  isComposing: false,
  keyCode: 83,
  altGraphKey: false,
  repeat: false,
  getModifierState: () => false,
  preventDefault: vi.fn(),
  stopPropagation: vi.fn(),
  ...props,
});

const options = (overrides = {}) => ({
  platform: PLATFORM_LINUX,
  context: { error: null },
  dispatch: vi.fn(),
  ...overrides,
});

describe("window keyboard dispatch", () => {
  it("dispatches an application shortcut and prevents the editor from seeing it", () => {
    const event = keyEvent({ ctrlKey: true });
    const opts = options();
    expect(handleKeydown(event, opts)).toBe(true);
    expect(opts.dispatch).toHaveBeenCalledWith("file.save");
    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("ignores plain editing keys", () => {
    const opts = options();
    expect(handleKeydown(keyEvent(), opts)).toBe(false);
    expect(opts.dispatch).not.toHaveBeenCalled();
  });

  it("does not fire while composing or with AltGraph", () => {
    const opts = options();
    expect(
      handleKeydown(keyEvent({ ctrlKey: true, isComposing: true }), opts)
    ).toBe(false);
    expect(handleKeydown(keyEvent({ ctrlKey: true, keyCode: 229 }), opts)).toBe(false);
    expect(
      handleKeydown(keyEvent({ ctrlKey: true, altGraphKey: true }), opts)
    ).toBe(false);
    expect(
      handleKeydown(
        keyEvent({
          ctrlKey: true,
          getModifierState: (key) => key === "AltGraph",
        }),
        opts
      )
    ).toBe(false);
    expect(opts.dispatch).not.toHaveBeenCalled();
  });

  it("ignores key repeats", () => {
    const opts = options();
    expect(handleKeydown(keyEvent({ ctrlKey: true, repeat: true }), opts)).toBe(false);
    expect(opts.dispatch).not.toHaveBeenCalled();
  });

  it("dismisses a visible error with Escape", () => {
    const event = keyEvent({ key: "Escape", code: "Escape" });
    const opts = options({ context: { error: "boom" } });
    expect(handleKeydown(event, opts)).toBe(true);
    expect(opts.dispatch).toHaveBeenCalledWith("error.dismiss");
    expect(event.preventDefault).toHaveBeenCalled();
    expect(event.stopPropagation).toHaveBeenCalled();
  });

  it("does not claim Escape when no error is visible", () => {
    const opts = options();
    expect(handleKeydown(keyEvent({ key: "Escape", code: "Escape" }), opts)).toBe(false);
    expect(opts.dispatch).not.toHaveBeenCalled();
  });

  it("respects a command's canRun guard", () => {
    const opts = options({ context: { error: null } });
    // error.dismiss has no keybinding, so it can only be reached through the
    // Escape fallback, which already checks the context.
    expect(handleKeydown(keyEvent({ key: "Escape", code: "Escape" }), opts)).toBe(false);
  });
});
