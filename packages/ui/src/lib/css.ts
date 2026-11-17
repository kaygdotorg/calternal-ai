/**
 * Resolves a length token (`--bar-h`, `--side-w`) to pixels. Custom properties read back unresolved (`calc(52px * var(--ui-scale, 1))`), so a probe
 * element takes the value as a width and the browser does the arithmetic. Cost: one forced layout of a 0-height element per call; call on change, not per frame.
 */
export function cssPx(name: string, fallback = 0): number {
  const probe = document.createElement("div");
  probe.style.cssText = `position:absolute;visibility:hidden;pointer-events:none;height:0;width:var(${name})`;
  document.body.append(probe);
  const w = probe.getBoundingClientRect().width;
  probe.remove();
  return w || fallback;
}
