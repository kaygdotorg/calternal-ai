/**
 * The keystroke probe: with `?test=1` in the address, the time from each key press to the next frame after the field changed is recorded in `window.__keys` (milliseconds).
 * The budgets (docs/editor-port-plan.md section 3) are measured with it, not claimed. Off by default; when off, `startProbe` returns immediately and adds no listener.
 */
export function probeOn(): boolean {
  try { return new URLSearchParams(location.search).get("test") === "1"; } catch { return false; }
}

/** Starts recording key-to-paint times on `el`. Returns the stop function. */
export function startProbe(el: HTMLElement): () => void {
  if (!probeOn()) return () => {};
  const w = window as unknown as { __keys?: number[] };
  w.__keys ??= [];
  let t0 = 0;
  const down = () => { t0 = performance.now(); };
  const input = () => { const t = t0 || performance.now(); requestAnimationFrame(() => w.__keys!.push(performance.now() - t)); t0 = 0; };
  el.addEventListener("keydown", down, true);
  el.addEventListener("input", input);
  return () => { el.removeEventListener("keydown", down, true); el.removeEventListener("input", input); };
}
