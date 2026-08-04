import { describe, expect, it } from "vitest";
import { isStaleResponse } from "./seq.js";

describe("isStaleResponse (§7.5 stale-response guard)", () => {
  it("drops a response older than the latest request sent", () => {
    expect(isStaleResponse(2, 3)).toBe(true);
  });

  it("keeps the response to the latest request", () => {
    expect(isStaleResponse(3, 3)).toBe(false);
  });

  it("keeps a response that cannot be older than the latest request", () => {
    expect(isStaleResponse(4, 3)).toBe(false);
  });

  it("keeps the very first response", () => {
    expect(isStaleResponse(1, 1)).toBe(false);
  });
});
