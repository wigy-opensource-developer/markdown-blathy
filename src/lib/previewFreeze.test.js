// The preview freeze: the paused/owed state, the stashed render and the
// close-time catch-up ordering, wrapped around the debounced renderer so the
// App only wires three call sites together (see App.jsx).
//
// Contracts the tests pin down: while frozen nothing renders; changes
// collapse into ONE render on close; a render that finished meanwhile is
// shown (not re-done) unless a newer change supersedes it; opening mid-
// debounce must not lose the scheduled render.
import { describe, expect, it, vi } from "vitest";
import { createPreviewFreeze } from "./previewFreeze.js";

/** The freeze wired to spy render side-effects. */
const make = () => {
  const refresh = vi.fn();
  const cancel = vi.fn();
  const apply = vi.fn();
  return { refresh, cancel, apply, freeze: createPreviewFreeze({ refresh, cancel, apply }) };
};

describe("createPreviewFreeze", () => {
  it("schedules a render immediately while open", () => {
    const { refresh, freeze } = make();
    freeze.request();
    expect(refresh).toHaveBeenCalledOnce();
  });

  it("applies a finished render directly while open", () => {
    const { apply, freeze } = make();
    freeze.request();
    freeze.started();
    freeze.finish("pages-A");
    expect(apply).toHaveBeenCalledWith("pages-A");
  });

  it("opening freezes the preview, dropping the pending debounce", () => {
    const { cancel, freeze } = make();
    freeze.open();
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("requests while frozen do not schedule", () => {
    const { refresh, freeze } = make();
    freeze.open();
    freeze.request();
    expect(refresh).not.toHaveBeenCalled();
  });

  it("stashes a render that lands while frozen", () => {
    const { apply, freeze } = make();
    freeze.request();
    freeze.started(); // render in flight before the popover opened
    freeze.open();
    freeze.finish("pages-A"); // it completes frozen
    expect(apply).not.toHaveBeenCalled();
  });

  it("closing shows the stashed render without re-rendering", () => {
    const { refresh, apply, freeze } = make();
    freeze.request();
    freeze.started();
    refresh.mockClear(); // forget the scheduling the request() above did
    freeze.open();
    freeze.finish("pages-A");
    freeze.close();
    expect(apply).toHaveBeenCalledWith("pages-A");
    expect(refresh).not.toHaveBeenCalled();
  });

  it("collapses many frozen requests into one catch-up render", () => {
    const { refresh, apply, freeze } = make();
    freeze.open();
    freeze.request();
    freeze.request();
    freeze.request(); // e.g. repeated Applys
    freeze.close();
    expect(refresh).toHaveBeenCalledOnce();
    expect(apply).not.toHaveBeenCalled();
  });

  it("a newer change supersedes the stashed render on close", () => {
    const { refresh, apply, freeze } = make();
    freeze.request();
    freeze.started();
    refresh.mockClear(); // forget the pre-open scheduling
    freeze.open();
    freeze.finish("stale"); // landed under the popover
    freeze.request(); // …but the user hit Apply afterwards
    freeze.close();
    expect(refresh).toHaveBeenCalledOnce(); // one render for the new markdown
    expect(apply).not.toHaveBeenCalled(); // the stale stash is dropped
  });

  it("a render cancelled mid-debounce stays owed and re-runs on close", () => {
    const { refresh, freeze } = make();
    freeze.request(); // scheduled, started() not reached yet
    freeze.open(); // …dropped by the cancel
    freeze.close();
    expect(refresh).toHaveBeenCalledTimes(2); // once deferred, once catch-up
  });

  it("closing without any change or stash renders and applies nothing", () => {
    const { refresh, apply, freeze } = make();
    freeze.request();
    freeze.started(); // the render fired and completed before opening
    freeze.finish("pages");
    freeze.open();
    freeze.close();
    expect(refresh).toHaveBeenCalledTimes(1); // only the pre-open request
    expect(apply).toHaveBeenCalledTimes(1); // only the pre-open finish
  });

  it("a close without an open is inert", () => {
    // The popover widget notifies its closed state on mount; an unpaired
    // close must not double the render a request() has already scheduled,
    // even one whose started() has not fired yet. Only a pause can owe.
    const { refresh, apply, freeze } = make();
    freeze.request();
    refresh.mockClear(); // forget that scheduling
    freeze.close();
    expect(refresh).not.toHaveBeenCalled();
    expect(apply).not.toHaveBeenCalled();
  });

  it("re-opens reuse the same freeze", () => {
    const { refresh, freeze } = make();
    freeze.open();
    freeze.request();
    freeze.close();
    expect(refresh).toHaveBeenCalledOnce();
    freeze.open();
    freeze.request();
    freeze.close();
    expect(refresh).toHaveBeenCalledTimes(2);
  });
});
