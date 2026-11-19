/**
 * `perFrame(fn)`: a function that runs at most once per frame, with the arguments of its last call, and never while input is waiting (a held key's next event: the page answers the person first; after
 * a few frames of waiting it runs anyway, so a flood of keys cannot starve updates for ever). The change feeds of Threads and Notes go through it: a churn of several changes a second is one list update a
 * frame, not one per change inside a key's task.
 */
export function perFrame<A extends unknown[]>(fn: (...a: A) => void): (...a: A) => void {
  let last: A | null = null, raf = 0, waited = 0;
  const pending = (): boolean => { try { return !!(navigator as Navigator & { scheduling?: { isInputPending?: (o?: { includeContinuous?: boolean }) => boolean } }).scheduling?.isInputPending?.({ includeContinuous: true }); } catch { return false; } };
  const run = () => {
    raf = 0;
    if (pending() && waited++ < 4) { raf = requestAnimationFrame(run); return; }
    waited = 0;
    const a = last; last = null;
    if (a) fn(...a);
  };
  return (...a: A) => { last = a; if (!raf) raf = requestAnimationFrame(run); };
}
