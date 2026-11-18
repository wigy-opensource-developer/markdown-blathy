// Unsaved-changes guard (M9 task 3): the state machine behind the
// Save/Discard/Cancel prompt for Close, New and Open.
//
// A request either runs at once (clean document) or waits for the user's
// choice; choosing Save resumes the *same* request only after the save
// actually happened, so "close the window" / "open the file I clicked" is
// never lost to a Save As that was cancelled. A failed save keeps the request
// pending, so the modal stays open for a retry.
//
// Pure and injected: App supplies `isDirty`, `save` (returns whether the
// document reached disk) and `perform` (runs the request's action), and
// mirrors the pending state into React through `onChange`.

export const CHOICE_SAVE = "save";
export const CHOICE_DISCARD = "discard";
export const CHOICE_CANCEL = "cancel";

export function createUnsavedGuard({
  isDirty,
  save,
  perform,
  onChange = () => {},
}) {
  let pending = null;

  /** Drop the request and (when it survived) run its action. */
  function settle(action) {
    pending = null;
    onChange(false);
    if (action) perform(action);
    return action;
  }

  return {
    /** Ask to run `action`. Returns `ran`, `pending` or `busy`.
     *  A second request while one is pending is ignored: the first one owns
     *  the modal, and replacing it could silently drop the user's intent. */
    request(action) {
      if (pending) return "busy";
      if (!isDirty()) {
        perform(action);
        return "ran";
      }
      pending = action;
      onChange(true);
      return "pending";
    },

    /** A modal choice. `cancel` drops the request; `save` resumes it only
     *  when `save()` succeeds; `discard` resumes it immediately. Returns the
     *  action that ran, or null when nothing did. */
    async choose(choice) {
      if (!pending) return null;
      const action = pending;
      if (choice === CHOICE_CANCEL) return settle(null);
      if (choice === CHOICE_SAVE && !(await save())) return null;
      return settle(action);
    },

    /** The action waiting for a choice, or null. */
    pending() {
      return pending;
    },
  };
}
