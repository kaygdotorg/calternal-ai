// Dragging a sheet down by its grabber (IN6, #469, #660): the one way a phone sheet is pulled closed, shared
// by every sheet (SheetBody for the bar's sheets, and the sheets of their own such as the invite sheet).
// The sheet follows the finger 1:1 downwards and stretches upwards (a rubber band). On release the owner
// is told the distance and how fast the finger was moving in its last 60 ms (pt per ms: a flick of
// 600 pt/s is 0.6), and decides (`shouldClose` is the house rule).

export type SheetDrag = (dy: number, done: boolean, velocity: number) => void;

/** The house rule: a flick of at least 600 pt/s, or past 40% of the sheet's height, closes it. */
export const shouldClose = (dy: number, velocity: number, height: number) => velocity >= 0.6 || dy > height * 0.4;

/** Put on the grabber strip (`touch-action: none`). */
export function sheetDrag(node: HTMLElement, ondrag: SheetDrag | undefined) {
  let tell = ondrag;
  let start = { y: 0, t: 0 };
  let dragging = false;
  let trail: { y: number; t: number }[] = [];
  const down = (e: PointerEvent) => {
    node.setPointerCapture(e.pointerId);
    dragging = true;
    start = { y: e.clientY, t: performance.now() };
    trail = [start];
  };
  const move = (e: PointerEvent) => {
    if (!dragging) return;
    const now = performance.now();
    trail.push({ y: e.clientY, t: now });
    while (trail.length > 2 && now - trail[0]!.t > 60) trail.shift();
    const d = e.clientY - start.y;
    tell?.(d > 0 ? d : -Math.pow(-d, 0.6), false, 0);
  };
  const up = (e: PointerEvent) => {
    if (!dragging) return;
    dragging = false;
    const a = trail[0] ?? start;
    tell?.(e.clientY - start.y, true, (e.clientY - a.y) / Math.max(performance.now() - a.t, 8));
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  return {
    update(next: SheetDrag | undefined) {
      tell = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", up);
      node.removeEventListener("pointercancel", up);
    },
  };
}
