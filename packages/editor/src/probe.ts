/**
 * The keystroke probe: with `?test=1` in the address, two times per key are recorded in milliseconds: `window.__work` (from the key press until the app's own update for it has run: the
 * input handlers and the framework's effects, which is the work the 8 ms budget is about) and `window.__keys` (from the key press to the next frame, which also waits for the frame boundary,
 * so a key that lands just after a frame costs up to a frame more whatever the work was).
 * The budgets (docs/editor-port-plan.md section 3) are measured with it, not claimed. Off by default; when off, `startProbe` returns immediately and adds no listener.
 */
export function probeOn(): boolean {
  try { return new URLSearchParams(location.search).get("test") === "1"; } catch { return false; }
}

/** Starts recording key-to-paint times on `el`. Returns the stop function. */
export function startProbe(el: HTMLElement): () => void {
  if (!probeOn()) return () => {};
  const w = window as unknown as { __keys?: number[]; __work?: number[] };
  w.__keys ??= [];
  w.__work ??= [];
  let t0 = 0;
  const down = () => { t0 = performance.now(); };
  const input = () => {
    const t = t0 || performance.now();
    t0 = 0;
    // Two microtask hops run after the framework's own flush for this key (queued during the input handlers), so this is the key's script work with no scheduling delay in it.
    queueMicrotask(() => queueMicrotask(() => queueMicrotask(() => w.__work!.push(performance.now() - t))));
    requestAnimationFrame(() => w.__keys!.push(performance.now() - t));
  };
  el.addEventListener("keydown", down, true);
  el.addEventListener("input", input);
  return () => { el.removeEventListener("keydown", down, true); el.removeEventListener("input", input); };
}
