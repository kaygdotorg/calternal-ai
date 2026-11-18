// All selectors mounted together read their options before any capsule writes a width.
// Otherwise each new capsule invalidates layout before the next selector's offsetLeft.
/** Makes a batcher: every `(read, write)` pair scheduled before `schedule` runs has all its reads done, then all its writes, so many selectors measuring at once cause one layout, not one each. Returns the function that adds a pair (and cancels it when called back). */
export function measurementBatch(schedule: (flush: () => void) => void) {
  type Job = { read: () => void; write: () => void; active: boolean };
  let pending: Job[] = [];
  return (read: () => void, write: () => void) => {
    const job = { read, write, active: true };
    if (!pending.length) schedule(() => {
      const jobs = pending;
      pending = [];
      for (const job of jobs) if (job.active) job.read();
      for (const job of jobs) if (job.active) job.write();
    });
    pending.push(job);
    return () => { job.active = false; };
  };
}

// Mount measurements still finish before first paint; resize measurements share one rAF.
/** The batcher for what must finish before the first paint (a microtask). */
export const measureOnMount = measurementBatch(queueMicrotask);
/** The batcher for later measurements (a resize, a font load): one animation frame. */
export const measureOnFrame = measurementBatch((flush) => { requestAnimationFrame(flush); });
