// Delayed-visibility state machine behind the "Rendering…" preview note.
//
// A keystroke render usually lands in ~300-400 ms; a spinner flashing for
// that would be noise. But a big document can pend for seconds and the user
// cannot tell "settled" from "still cooking". So: `start()` when a render
// begins, `stop()` when it lands — and only notify after the request has
// been pending for `delayMs` continuously. Rapid keystrokes chain starts;
// the delay arms once, from the first pending render.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRenderStatus } from "./renderStatus.js";

const make = (delayMs = 350) => {
  const notify = vi.fn();
  const status = createRenderStatus({ delayMs, notify });
  return { notify, status };
};

describe("createRenderStatus", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("stays silent for renders that finish inside the delay", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(200);
    status.stop();
    vi.advanceTimersByTime(1000);
    expect(notify).not.toHaveBeenCalled();
  });

  it("shows when a render is still pending after the delay", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(350);
    expect(notify).toHaveBeenCalledWith(true);
  });

  it("hides again when the pending render lands", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(350);
    status.stop();
    expect(notify).toHaveBeenLastCalledWith(false);
    expect(notify).toHaveBeenCalledTimes(2);
  });

  it("stop before the note is visible notifies nothing", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(100);
    status.stop();
    status.stop(); // an extra stop is inert
    expect(notify).not.toHaveBeenCalled();
  });

  it("keeps one deadline across chained starts (keystroke storms)", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(200);
    status.start(); // next keystroke re-render — same pending episode
    vi.advanceTimersByTime(150); // 350 ms since the FIRST start
    expect(notify).toHaveBeenCalledWith(true);
  });

  it("re-arms for a fresh episode after hiding", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(350);
    status.stop(); // visible → hidden
    notify.mockClear();
    status.start(); // a later edit
    vi.advanceTimersByTime(350);
    expect(notify).toHaveBeenCalledWith(true);
  });

  it("stays visible while chained starts arrive after the delay", () => {
    const { notify, status } = make();
    status.start();
    vi.advanceTimersByTime(350);
    notify.mockClear();
    status.start();
    status.stop();
    expect(notify).toHaveBeenCalledWith(false);
  });
});
