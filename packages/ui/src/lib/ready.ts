/**
 * A floating surface (a popover, a menu, a sheet) must never be visible before its material is ready (owner rule, 2026-11-17: no transparent
 * popover that then blurs). The first paint of a `backdrop-filter` surface can show its fill without the blur, and the adaptive tint
 * (`--mat-a`, lib/material.ts) is written a frame after mount. This action marks the surface `data-surface` (hidden, slightly scaled down)
 * and `data-ready` after two animation frames, when both are in place; base.css then eases it in with a transition, which starts only then,
 * is interruptible, and never delays input. With the keyboard in use the transition has no duration (lib/input.ts), so the end state is immediate.
 */
export function ready(node: HTMLElement) {
  node.setAttribute("data-surface", "");
  let f = requestAnimationFrame(() => {
    f = requestAnimationFrame(() => node.setAttribute("data-ready", ""));
  });
  return { destroy: () => cancelAnimationFrame(f) };
}
