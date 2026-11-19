/**
 * `perFrame(fn)`: a function that runs at most once per frame, with the arguments of its last call, and never while input is waiting (a held key's next event: the page answers the person first; after
 * a few frames of waiting it runs anyway, so a flood of keys cannot starve updates for ever). The change feeds of Threads and Notes go through it: a churn of several changes a second is one list update a
 * frame, not one per change inside a key's task. While a navigation key repeats (`navHold`) the latest call is HELD and applied once when the cursor settles, or after 500 ms at most.
 */
import { navHold } from "@calternal/ui";

/** How long after the cursor settles a held list update is applied. */
const AFTER_SETTLE_MS = 100;

export function perFrame<A extends unknown[]>(fn: (...a: A) => void): (...a: A) => void {
  let last: A | null = null, raf = 0, waited = 0;
  const pending = (): boolean => { try { return !!(navigator as Navigator & { scheduling?: { isInputPending?: (o?: { includeContinuous?: boolean }) => boolean } }).scheduling?.isInputPending?.({ includeContinuous: true }); } catch { return false; } };
  const run = () => {
    raf = 0;
    if (navHold.held) { afterHold(); return; } // held: applied after the release
    if (pending() && waited++ < 4) { raf = requestAnimationFrame(run); return; }
    waited = 0;
    const a = last; last = null;
    if (a) fn(...a);
  };
  // When the cursor settles the pane's own load starts: the held update lands a moment after it (not in the same frames), unless more keys arrive first.
  // Subscribed only while a call is held (and dropped when the hold ends), so a long-lived page that makes many of these (a store per space) accumulates no subscribers or closures.
  let unsub: (() => void) | null = null;
  const afterHold = () => {
    if (unsub) return;
    unsub = navHold.subscribe(() => {
      unsub?.(); unsub = null;
      // When the cursor settles the pane's own load starts: the held update lands a moment after it (not in the same frames), unless more keys arrive first.
      if (last) setTimeout(() => { if (!last) return; if (navHold.held) afterHold(); else if (!raf) raf = requestAnimationFrame(run); }, AFTER_SETTLE_MS);
    });
  };
  return (...a: A) => { last = a; if (navHold.held) { afterHold(); return; } if (!raf) raf = requestAnimationFrame(run); };
}
