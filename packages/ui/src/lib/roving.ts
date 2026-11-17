/** Roving focus for toolbars and tab lists: arrows move focus between the children that match `selector`; Home and End jump. */

/** Svelte action. `axis` picks the arrow keys. Cost: one query per key press. */
export function roving(node: HTMLElement, opts: { selector: string; axis?: "x" | "y" | "both" } = { selector: "button:not(:disabled)" }) {
  let o = opts;
  const keys = () => (o.axis === "y" ? ["ArrowUp", "ArrowDown"] : o.axis === "both" ? ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"] : ["ArrowLeft", "ArrowRight"]);
  function onKey(e: KeyboardEvent) {
    const items = [...node.querySelectorAll<HTMLElement>(o.selector)];
    if (!items.length) return;
    const i = items.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (e.key === "Home") next = 0;
    else if (e.key === "End") next = items.length - 1;
    else if (keys().includes(e.key)) {
      const back = e.key === "ArrowLeft" || e.key === "ArrowUp";
      next = (i + (back ? -1 : 1) + items.length) % items.length;
    }
    if (next >= 0) {
      e.preventDefault();
      items[next]!.focus();
    }
  }
  node.addEventListener("keydown", onKey);
  return {
    update(n: typeof opts) {
      o = n;
    },
    destroy() {
      node.removeEventListener("keydown", onKey);
    },
  };
}
