/**
 * Pull to refresh (touch, where a list supports it): pulling down on a scroller that is at its top shows a spinner that follows the finger; releasing past 64 px
 * calls `refresh` and holds the spinner until it settles. The scroller keeps its own scrolling (the gesture only starts at scrollTop 0 and a downward drag).
 * Cost: pointer handlers on the scroller; one transform on one small element while pulling.
 */
export interface PullOptions { refresh: () => Promise<unknown> | void }
const THRESHOLD = 64;

/** Svelte action. */
export function pull(node: HTMLElement, o: PullOptions) {
  const ind = document.createElement("div");
  ind.className = "pull-ind";
  ind.setAttribute("aria-hidden", "true");
  ind.innerHTML = '<svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 4a8 8 0 1 0 8 8"/></svg>';
  Object.assign(ind.style, { position: "absolute", left: "50%", top: "0", zIndex: "5", width: "32px", height: "32px", marginLeft: "-16px", display: "grid", placeItems: "center", borderRadius: "50%", background: "var(--paper)", boxShadow: "var(--shadow-float)", color: "var(--ink-2)", opacity: "0", pointerEvents: "none", translate: "0 -40px" });
  node.style.position ||= "relative";
  node.append(ind);
  let y0 = 0, pulling = false, busy = false, d = 0;
  const show = (v: number, spin = false) => { ind.style.opacity = String(Math.min(1, v / THRESHOLD)); ind.style.translate = `0 ${Math.min(v, 96) - 40}px`; ind.style.rotate = spin ? "" : `${v * 3}deg`; ind.style.animation = spin ? "spin 800ms linear infinite" : ""; };
  const down = (e: PointerEvent) => { if (e.pointerType !== "touch" || busy || node.scrollTop > 0) return; y0 = e.clientY; pulling = false; d = 0; };
  const move = (e: PointerEvent) => {
    if (e.pointerType !== "touch" || !y0) return;
    const dy = e.clientY - y0;
    if (!pulling && dy > 10 && node.scrollTop <= 0) pulling = true;
    if (pulling) { d = dy * 0.5; show(d); }
  };
  const up = async (e: PointerEvent) => {
    if (e.pointerType !== "touch" || !y0) return;
    y0 = 0;
    if (!pulling) return;
    pulling = false;
    if (d >= THRESHOLD / 2) {
      busy = true; show(THRESHOLD, true);
      try { await o.refresh(); } finally { busy = false; }
    }
    ind.style.transition = "opacity 200ms, translate 200ms"; show(0);
    setTimeout(() => (ind.style.transition = ""), 250);
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  return { update(n: PullOptions) { o = n; }, destroy() { node.removeEventListener("pointerdown", down); node.removeEventListener("pointermove", move); node.removeEventListener("pointerup", up); node.removeEventListener("pointercancel", up); ind.remove(); } };
}
