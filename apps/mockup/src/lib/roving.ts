/**
 * Keyboard travel through a list as ONE Tab stop (a roving tabindex): the list has a single focusable item at a time (the last one used, else the first); ArrowDown and ArrowUp (and ArrowRight and ArrowLeft inside a strip) move to the next and the previous,
 * Home and End to the first and last, Enter and Space press the focused item (a button does it by itself), Escape hands the focus back to the page. Everything else focusable in the list (a section's small buttons, a row's pin, a task's check box) is out of the
 * Tab order and reached by a key on its row instead (`m` or the menu key on a heading, `p` on a card row, Space on a task), so the whole sidebar costs a handful of Tab stops, not one per row. It follows the list as it changes (sections folded, rows added).
 */
export interface RovingOptions { /** What counts as an item. */ selector: string; /** Focusable things that are not items: they leave the Tab order. */ extras?: string }
export function roving(node: HTMLElement, o: RovingOptions) {
  let active: HTMLElement | null = null;
  const visible = (e: HTMLElement) => e.getClientRects().length > 0 && !e.closest("[inert]") && !e.hasAttribute("disabled");
  const items = () => [...node.querySelectorAll<HTMLElement>(o.selector)].filter(visible);
  const apply = () => {
    const l = items(); if (!l.length) return;
    if (!active || !l.includes(active)) active = l[0]!;
    for (const e of l) e.tabIndex = e === active ? 0 : -1;
    if (o.extras) for (const e of node.querySelectorAll<HTMLElement>(o.extras)) if (!l.includes(e)) e.tabIndex = -1;
  };
  const onfocus = (ev: FocusEvent) => { const t = (ev.target as HTMLElement).closest<HTMLElement>(o.selector); if (t && items().includes(t)) { active = t; apply(); } };
  const onkey = (ev: KeyboardEvent) => {
    const t = ev.target as HTMLElement; if (!t.matches(o.selector) || ev.altKey || ev.ctrlKey || ev.metaKey) return;
    const l = items(), i = l.indexOf(t); if (i < 0) return;
    let to = -1;
    if (ev.key === "ArrowDown" || (ev.key === "ArrowRight" && !!t.closest(".strip, [role=tablist]"))) to = Math.min(l.length - 1, i + 1);
    else if (ev.key === "ArrowUp" || (ev.key === "ArrowLeft" && !!t.closest(".strip, [role=tablist]"))) to = Math.max(0, i - 1);
    else if (ev.key === "Home") to = 0; else if (ev.key === "End") to = l.length - 1;
    else if (ev.key === "Escape") { (document.querySelector<HTMLElement>("[data-win][data-focused=true] .scroll") ?? document.body).focus?.(); (document.activeElement as HTMLElement)?.blur?.(); return; }
    else if (ev.key === " " && t.closest("[data-task]")) { t.closest("[data-task]")!.querySelector<HTMLElement>(".ck")?.click(); ev.preventDefault(); return; }
    if (to < 0) return;
    ev.preventDefault(); active = l[to]!; apply(); active.focus(); active.scrollIntoView({ block: "nearest", inline: "nearest" });
  };
  const mo = new MutationObserver(() => queueMicrotask(apply));
  node.addEventListener("focusin", onfocus); node.addEventListener("keydown", onkey); mo.observe(node, { childList: true, subtree: true, attributes: true, attributeFilter: ["inert", "disabled"] }); apply();
  return { update(n: RovingOptions) { o = n; apply(); }, destroy() { node.removeEventListener("focusin", onfocus); node.removeEventListener("keydown", onkey); mo.disconnect(); } };
}
