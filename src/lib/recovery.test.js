import { describe, expect, it } from "vitest";
import {
  AUTOSAVE_INTERVAL_MS,
  recoveryDecision,
} from "./recovery.js";

const SNAPSHOT = {
  id: "0123456789abcdef",
  markdown: "# Draft",
  path: "/home/u/notes.md",
  savedAt: 1_760_000_000,
};

describe("AUTOSAVE_INTERVAL_MS", () => {
  it("snapshots a dirty document every 30 seconds", () => {
    expect(AUTOSAVE_INTERVAL_MS).toBe(30_000);
  });
});

describe("recoveryDecision", () => {
  it("does nothing without a snapshot", () => {
    expect(recoveryDecision({ snapshot: null, pristine: true })).toBe("none");
  });

  it("offers the snapshot while the editor is pristine", () => {
    expect(recoveryDecision({ snapshot: SNAPSHOT, pristine: true })).toBe("offer");
  });

  it("keeps the snapshot when the user already typed", () => {
    expect(recoveryDecision({ snapshot: SNAPSHOT, pristine: false })).toBe("keep");
  });

  it("offers a CLI-opened document's own snapshot", () => {
    expect(
      recoveryDecision({
        snapshot: SNAPSHOT,
        pristine: true,
        expectedPath: "/home/u/notes.md",
      })
    ).toBe("offer");
  });

  it("does not hijack a CLI-opened document with another document's snapshot", () => {
    expect(
      recoveryDecision({
        snapshot: SNAPSHOT,
        pristine: true,
        expectedPath: "/home/u/other.md",
      })
    ).toBe("keep");
  });
});
