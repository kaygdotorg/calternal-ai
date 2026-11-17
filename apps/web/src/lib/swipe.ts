/**
 * Touch gestures on a list row (docs/native-ui-study.md 13.2): a horizontal swipe slides the row left by `reveal` px to show its actions (two 50 px round
 * buttons, 10 apart), and a long press (500 ms without moving) asks for the context menu. Touch only; a mouse never starts either. The row follows the finger
 * with a transform only (no layout), settles open or closed on release at the form's sheet motion, and a swipe never counts as a click on the row's link.
 * Cost: pointer handlers on the row; nothing runs while it is idle.
 */
export interface SwipeOptions { reveal: number; onlong?: (x: number, y: number) => void }

/** Svelte action. The row reads `--sx` (its current offset); `data-open` says whether it rests open. */
export function swipe(node: HTMLElement, o: SwipeOptions) {
  let x0 = 0, y0 = 0, base = 0, dragging = false, timer = 0, swallow = false;
  const set = (v: number) => node.style.setProperty("--sx", `${v}px`);
  const cancelLong = () => { clearTimeout(timer); timer = 0; };
  const down = (e: PointerEvent) => {
    if (e.pointerType !== "touch") return;
    x0 = e.clientX; y0 = e.clientY; dragging = false;
    base = node.dataset.open === "true" ? -o.reveal : 0;
    cancelLong();
    if (o.onlong) timer = window.setTimeout(() => { swallow = true; o.onlong!(x0, y0); }, 500);
  };
  const move = (e: PointerEvent) => {
    if (e.pointerType !== "touch" || (!x0 && !y0)) return;
    const dx = e.clientX - x0, dy = e.clientY - y0;
    if (!dragging && Math.abs(dx) > 8 && Math.abs(dx) > Math.abs(dy) * 1.5) { dragging = true; cancelLong(); node.classList.add("dragging"); node.setPointerCapture(e.pointerId); }
    else if (!dragging && (Math.abs(dy) > 8 || Math.abs(dx) > 8)) cancelLong();
    if (dragging) set(Math.min(0, Math.max(-o.reveal - 24, base + dx)));
  };
  const up = (e: PointerEvent) => {
    if (e.pointerType !== "touch") return;
    cancelLong();
    if (dragging) {
      const cur = parseFloat(node.style.getPropertyValue("--sx")) || 0;
      const open = cur < -o.reveal / 2;
      node.dataset.open = String(open);
      node.classList.remove("dragging");
      set(open ? -o.reveal : 0);
      swallow = true;
    }
    dragging = false; x0 = y0 = 0;
  };
  const click = (e: Event) => { if (swallow) { e.preventDefault(); e.stopPropagation(); swallow = false; } };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", up);
  node.addEventListener("pointercancel", up);
  node.addEventListener("click", click, true);
  return {
    update(n: SwipeOptions) { o = n; },
    destroy() { cancelLong(); node.removeEventListener("pointerdown", down); node.removeEventListener("pointermove", move); node.removeEventListener("pointerup", up); node.removeEventListener("pointercancel", up); node.removeEventListener("click", click, true); },
  };
}
