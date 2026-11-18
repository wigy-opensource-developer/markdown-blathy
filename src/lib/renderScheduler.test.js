// The render scheduler: the contracts of the whole render cycle.
//
// Debounce: edits render only after a 200 ms pause, and each edit restarts
// the wait. Single flight: while a render is in flight nothing new starts
// or is even scheduled — edits only mark a catch-up owed. Catch-up: when
// the render settles, a new one starts iff there were edits since it began
// and the last edit is ≥200 ms old; newer edits arm the remaining debounce.
// Loads skip the debounce but stay single-flight (the in-flight response is
// invalidated). The modal freeze: nothing renders while paused, finished
// results stash, and close catches up exactly once.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRenderScheduler } from "./renderScheduler.js";

/** The scheduler wired to a controllable render: each call returns a
 *  promise the test resolves by hand (result, or null for dropped/failed). */
const make = () => {
  const pending = [];
  const render = vi.fn(() => {
    let resolve;
    const promise = new Promise((r) => {
      resolve = r;
    });
    pending.push(resolve);
    return promise;
  });
  const apply = vi.fn();
  const invalidate = vi.fn();
  const scheduler = createRenderScheduler({ render, apply, invalidate });
  return { scheduler, render, apply, invalidate, pending };
};

/** Let the scheduler's finish() run after a render promise resolves. */
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};

describe("createRenderScheduler", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  // — Debounce: only a pause renders.

  it("an edit renders after a 200 ms pause, not before", () => {
    const { scheduler, render } = make();
    scheduler.request();
    vi.advanceTimersByTime(199);
    expect(render).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(render).toHaveBeenCalledOnce();
  });

  it("each edit restarts the debounce", () => {
    const { scheduler, render } = make();
    scheduler.request();
    vi.advanceTimersByTime(150);
    scheduler.request();
    vi.advanceTimersByTime(199); // 349 ms total, but only 199 since the edit
    expect(render).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(render).toHaveBeenCalledOnce();
  });

  it("a burst of edits produces one render", () => {
    const { scheduler, render } = make();
    for (let i = 0; i < 20; i++) {
      scheduler.request();
      vi.advanceTimersByTime(60);
    }
    vi.advanceTimersByTime(200);
    expect(render).toHaveBeenCalledOnce();
  });

  // — Single flight: typing goes on, nothing is queued mid-render.

  it("edits during a render start nothing, even after a long pause", () => {
    const { scheduler, render } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // render #1 in flight
    for (let i = 0; i < 5; i++) {
      scheduler.request();
      vi.advanceTimersByTime(50);
    }
    vi.advanceTimersByTime(5000); // the user stopped typing long ago
    expect(render).toHaveBeenCalledOnce(); // …and still nothing new started
  });

  it("after the render lands, old edits start the catch-up at once", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight
    vi.advanceTimersByTime(100);
    scheduler.request(); // an edit during the render…
    vi.advanceTimersByTime(200); // …≥200 ms old by the time it lands
    pending[0]("pages-A");
    await flush();
    expect(apply).toHaveBeenCalledWith("pages-A");
    expect(render).toHaveBeenCalledTimes(2); // catch-up started immediately
  });

  it("after the render lands, recent edits arm the remaining debounce", async () => {
    const { scheduler, render, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight
    vi.advanceTimersByTime(1000);
    scheduler.request(); // the user typed 0 ms ago when #1 lands
    pending[0]("pages-A");
    await flush();
    expect(render).toHaveBeenCalledOnce(); // not yet — the pause must hold
    vi.advanceTimersByTime(199);
    expect(render).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(1);
    expect(render).toHaveBeenCalledTimes(2);
  });

  it("no edits during the render means no catch-up", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200);
    pending[0]("pages-A");
    await flush();
    vi.advanceTimersByTime(10000);
    expect(render).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledOnce();
  });

  it("many edits during one render collapse into one catch-up", async () => {
    const { scheduler, render, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200);
    for (let i = 0; i < 10; i++) {
      scheduler.request();
      vi.advanceTimersByTime(30);
    }
    vi.advanceTimersByTime(200);
    pending[0]("pages-A");
    await flush();
    expect(render).toHaveBeenCalledTimes(2);
  });

  it("a failed render still runs the catch-up check", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200);
    scheduler.request();
    vi.advanceTimersByTime(200);
    pending[0](null); // compile failed; App surfaced the error itself
    await flush();
    expect(apply).not.toHaveBeenCalled();
    expect(render).toHaveBeenCalledTimes(2); // the edit is not lost
  });

  it("a rejecting render is a failure, not a stuck cycle", async () => {
    const render = vi.fn(async () => {
      throw new Error("boom");
    });
    const apply = vi.fn();
    const scheduler = createRenderScheduler({ render, apply });
    scheduler.request();
    vi.advanceTimersByTime(200);
    await flush();
    expect(render).toHaveBeenCalledOnce();
    expect(apply).not.toHaveBeenCalled();
    scheduler.request(); // the cycle still accepts work afterwards
    vi.advanceTimersByTime(200);
    await flush();
    expect(render).toHaveBeenCalledTimes(2);
  });

  it("the catch-up render is itself single-flight", async () => {
    const { scheduler, render, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1
    scheduler.request();
    vi.advanceTimersByTime(200);
    pending[0]("A");
    await flush(); // #2 starts
    expect(render).toHaveBeenCalledTimes(2);
    scheduler.request(); // edits during #2…
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledTimes(2); // …queue nothing
    pending[1]("B");
    await flush();
    expect(render).toHaveBeenCalledTimes(3); // one catch-up for them
  });

  // — Loads: no debounce, but still single-flight.

  it("requestNow renders immediately, skipping the debounce", () => {
    const { scheduler, render, invalidate } = make();
    scheduler.requestNow();
    expect(render).toHaveBeenCalledOnce();
    expect(invalidate).toHaveBeenCalledOnce();
  });

  it("requestNow supersedes a pending keystroke debounce", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(100);
    scheduler.requestNow(); // the load includes the typed edits (live buffer)
    expect(render).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledOnce(); // no second render for the keystroke
    pending[0]("pages-A");
    await flush();
    expect(apply).toHaveBeenCalledWith("pages-A");
    expect(render).toHaveBeenCalledOnce();
  });

  it("requestNow during a render invalidates it and waits for it to settle", async () => {
    const { scheduler, render, invalidate, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight (the old document)
    scheduler.requestNow(); // a load
    expect(invalidate).toHaveBeenCalledOnce(); // #1's response will be dropped
    expect(render).toHaveBeenCalledOnce(); // …but no second compile yet
    vi.advanceTimersByTime(5000);
    expect(render).toHaveBeenCalledOnce();
    pending[0](null); // #1 lands stale
    await flush();
    expect(render).toHaveBeenCalledTimes(2); // the load renders at once
  });

  it("typing after a deferred load folds into the debounced catch-up", async () => {
    const { scheduler, render, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight
    scheduler.requestNow(); // load deferred behind #1
    scheduler.request(); // the user types into the loaded document
    pending[0](null);
    await flush();
    expect(render).toHaveBeenCalledOnce(); // the burst is not rendered into
    vi.advanceTimersByTime(200);
    expect(render).toHaveBeenCalledTimes(2); // one render after the pause
  });

  it("requestNow leaves nothing owed once the render started", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.requestNow();
    pending[0]("pages-A");
    await flush();
    scheduler.open();
    scheduler.close();
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledOnce(); // no phantom catch-up
    expect(apply).toHaveBeenCalledOnce();
  });

  // — The modal freeze: pause, owe, stash, one catch-up on close.

  it("open drops the pending debounce; close catches up once", () => {
    const { scheduler, render } = make();
    scheduler.request();
    scheduler.open();
    vi.advanceTimersByTime(1000);
    expect(render).not.toHaveBeenCalled();
    scheduler.close(); // the edit is long past its deadline → render at once
    expect(render).toHaveBeenCalledOnce();
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledOnce();
  });

  it("requests while frozen never render; close collapses them into one", () => {
    const { scheduler, render } = make();
    scheduler.open();
    scheduler.request();
    scheduler.request();
    scheduler.request();
    vi.advanceTimersByTime(1000);
    expect(render).not.toHaveBeenCalled();
    scheduler.close();
    expect(render).toHaveBeenCalledOnce();
  });

  it("a result landing while frozen stashes; close shows it without re-rendering", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight before the popover opened
    scheduler.open();
    pending[0]("pages-A");
    await flush();
    expect(apply).not.toHaveBeenCalled();
    scheduler.close();
    expect(apply).toHaveBeenCalledWith("pages-A");
    expect(render).toHaveBeenCalledOnce(); // shown, not re-done
  });

  it("a newer change supersedes the stashed result on close", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200);
    scheduler.open();
    pending[0]("stale");
    await flush();
    scheduler.request(); // the user hit Apply afterwards
    scheduler.close();
    expect(apply).not.toHaveBeenCalled(); // the stash is dropped
    vi.advanceTimersByTime(200); // the fresh edit debounces normally
    expect(render).toHaveBeenCalledTimes(2); // one render for the new text
  });

  it("an unpaired close is inert", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200);
    pending[0]("pages-A");
    await flush();
    scheduler.close(); // a widget notifying its initial closed state
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledOnce();
    expect(apply).toHaveBeenCalledOnce();
  });

  it("close before an in-flight render lands: apply it, then one catch-up", async () => {
    const { scheduler, render, apply, pending } = make();
    scheduler.request();
    vi.advanceTimersByTime(200); // #1 in flight
    scheduler.open();
    scheduler.request(); // an edit under the popover
    scheduler.close(); // owed, but #1 still flying → nothing starts yet
    vi.advanceTimersByTime(1000);
    expect(render).toHaveBeenCalledOnce();
    pending[0]("pages-A");
    await flush();
    expect(apply).toHaveBeenCalledWith("pages-A"); // latest completed preview
    expect(render).toHaveBeenCalledTimes(2); // the edit catches up once
  });

  it("requestNow while frozen catches up on close", () => {
    const { scheduler, render } = make();
    scheduler.open();
    scheduler.requestNow();
    vi.advanceTimersByTime(1000);
    expect(render).not.toHaveBeenCalled();
    scheduler.close();
    expect(render).toHaveBeenCalledOnce();
  });

  it("dispose drops the pending debounce", () => {
    const { scheduler, render } = make();
    scheduler.request();
    scheduler.dispose();
    vi.advanceTimersByTime(1000);
    expect(render).not.toHaveBeenCalled();
  });
});
