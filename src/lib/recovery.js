// Crash-recovery rules (M9 task 4): autosave cadence and the startup
// decision. Kept pure so the timing/pristine logic is testable without a
// browser; App wires them to the commands and the modal.

/** How often a dirty document is snapshotted (plus once immediately). */
export const AUTOSAVE_INTERVAL_MS = 30_000;

/**
 * What to do with a snapshot found at startup.
 *
 * - `none`  — nothing to offer.
 * - `keep`  — leave it for the next launch. Either the user already typed
 *             (the editor is no longer pristine, so replacing it would lose
 *             their work), or a CLI-opened document is not the one the
 *             snapshot belongs to (don't hijack the document they asked for).
 * - `offer` — show the recovery prompt; the snapshot stays on disk until the
 *             user chooses Discard or a save succeeds.
 */
export function recoveryDecision({ snapshot, pristine, expectedPath = null }) {
  if (!snapshot) return "none";
  if (!pristine) return "keep";
  if (expectedPath !== null && snapshot.path !== expectedPath) return "keep";
  return "offer";
}
