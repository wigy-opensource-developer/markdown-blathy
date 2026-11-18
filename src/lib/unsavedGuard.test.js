// The unsaved-changes state machine (src/lib/unsavedGuard.js): a clean
// document runs the request immediately; a dirty one waits for the modal,
// and a successful save resumes the exact request that was interrupted.
import { describe, expect, it, vi } from "vitest";
import {
  CHOICE_CANCEL,
  CHOICE_DISCARD,
  CHOICE_SAVE,
  createUnsavedGuard,
} from "./unsavedGuard.js";

/** A guard wired to spies; `dirty` and `saved` can be flipped per test. */
function harness({ dirty = false, saved = true } = {}) {
  const state = { dirty, saved, performed: [], changes: [] };
  const guard = createUnsavedGuard({
    isDirty: () => state.dirty,
    save: vi.fn(async () => state.saved),
    perform: (action) => state.performed.push(action),
    onChange: (pending) => state.changes.push(pending),
  });
  return { guard, state };
}

describe("createUnsavedGuard", () => {
  it("runs the request immediately when the document is clean", () => {
    const { guard, state } = harness({ dirty: false });
    expect(guard.request("close")).toBe("ran");
    expect(state.performed).toEqual(["close"]);
    expect(state.changes).toEqual([]);
  });

  it("holds the request while dirty and reports the pending state", () => {
    const { guard, state } = harness({ dirty: true });
    expect(guard.request("open")).toBe("pending");
    expect(guard.pending()).toBe("open");
    expect(state.changes).toEqual([true]);
    expect(state.performed).toEqual([]);
  });

  it("cancelling drops the request without running it", async () => {
    const { guard, state } = harness({ dirty: true });
    guard.request("new");
    expect(await guard.choose(CHOICE_CANCEL)).toBeNull();
    expect(guard.pending()).toBeNull();
    expect(state.performed).toEqual([]);
    expect(state.changes).toEqual([true, false]);
  });

  it("discarding runs the interrupted request at once", async () => {
    const { guard, state } = harness({ dirty: true });
    guard.request("open");
    expect(await guard.choose(CHOICE_DISCARD)).toBe("open");
    expect(state.performed).toEqual(["open"]);
  });

  it("saving resumes the exact interrupted request", async () => {
    const { guard, state } = harness({ dirty: true });
    guard.request("close");
    expect(await guard.choose(CHOICE_SAVE)).toBe("close");
    expect(state.performed).toEqual(["close"]);
    expect(guard.pending()).toBeNull();
  });

  it("a cancelled or failed save keeps the request pending for a retry", async () => {
    const { guard, state } = harness({ dirty: true, saved: false });
    guard.request("close");
    expect(await guard.choose(CHOICE_SAVE)).toBeNull();
    expect(guard.pending()).toBe("close", "the user can still discard or retry");
    expect(state.performed).toEqual([]);
    expect(state.changes).toEqual([true], "the modal stays open");
  });

  it("ignores a second request while one is pending", () => {
    const { guard, state } = harness({ dirty: true });
    guard.request("close");
    expect(guard.request("open")).toBe("busy");
    expect(guard.pending()).toBe("close", "the first intent wins");
    expect(state.changes).toEqual([true]);
  });
});
