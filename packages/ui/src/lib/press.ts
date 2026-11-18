/**
 * The press of a list row: hold a row on a touch screen and the page dims, a preview of the item grows out of the row and the row's menu sits under it (as
 * Apple Notes does); right-click or the menu key opens the same menu without the preview. Ported from the existing app's press component (press.ts,
 * press-rows.ts, config.ts; the rule is its README's table), cut down to what a list needs: no drag and drop (the `drag` hook of a row's data is reserved and
 * unused), the menu list is the shared `Menu`.
 *
 * The finger's timeline: it moves before `LIFT_MS` and it is a scroll (the browser cancels the pointer); it holds `LIFT_MS` and the row lifts a little; it
 * holds `MENU_MS` and the preview and the menu open; it moves more than `STILL` px in between and the lift settles back and nothing opens.
 *
 * Cost: a list wears `use:pressRows` (four passive listeners on the scroller, nothing per row), each row `use:pressRow` (one WeakMap entry, no listeners). A row's
 * data is read when it is pressed; its look-alike is made `PRIME_MS` into the hold and the surface (components/PressSurface.svelte, loaded on the first hold) is
 * built after that; scrolling never runs any of it.
 */
import { tick } from "svelte";
import { lookalike } from "./lift";
import { pressState, type PressBag } from "./press-state.svelte";

export type { PressBag } from "./press-state.svelte";

/** A finger still down after this lifts the row. */
export const LIFT_MS = 250;
/** A finger still down after this opens the preview and the menu. */
export const MENU_MS = 500;
/** How far a finger may wander and still count as still, px. */
export const STILL = 8;
/** The row's look-alike is made this far into the hold, so the open itself only shows it. */
export const PRIME_MS = 120;
/** The row's lift: a little bigger, never more. */
export const LIFT_SCALE = 1.03;

const bags = new WeakMap<HTMLElement, () => PressBag>();
const reduced = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

/** A row that can be pressed: `get` returns what the press needs, read when it happens. */
export function pressRow(node: HTMLElement, get: () => PressBag) {
  bags.set(node, get);
  node.setAttribute("data-press", "");
  return { update(g: () => PressBag) { bags.set(node, g); }, destroy() { bags.delete(node); } };
}

let surface: Promise<void> | null = null;
/** Loads and mounts the surface once (the first hold on any list); later holds find it ready. */
function ensureSurface(): Promise<void> {
  return (surface ??= Promise.all([import("svelte"), import("../components/PressSurface.svelte")]).then(([{ mount }, mod]) => {
    mount(mod.default, { target: document.body });
  }));
}

/** Opens the menu of `row` now: with the preview (a touch hold) or alone at a point (right click, the key). */
export async function openPress(row: HTMLElement, preview: boolean, x: number, y: number, copy?: HTMLElement): Promise<void> {
  const get = bags.get(row);
  if (!get) return;
  await ensureSurface();
  const bag = get();
  await tick();
  pressState.cur = { row, bag, preview: preview && !!bag.preview, x, y, copy };
  pressState.stage = null;
}

/** Builds the card of `row` now, unseen (called a little into the hold): opening then only reveals it. */
async function stagePress(row: HTMLElement, copy: HTMLElement | undefined): Promise<void> {
  const get = bags.get(row);
  if (!get) return;
  await ensureSurface();
  if (bags.get(row) === get) pressState.stage = { row, bag: get(), copy };
}

/** The lifted row: its look-alike stands over the row (which is hidden meanwhile), a little bigger with a shadow, and follows the finger by a fraction of how far it has moved. */
function liftCopy(row: HTMLElement, copy: HTMLElement): { move(dx: number, dy: number): void; drop(): void; keep(): HTMLElement } {
  const r = row.getBoundingClientRect();
  copy.style.cssText += `;position:fixed;left:${r.left}px;top:${r.top}px;width:${r.width}px;height:${r.height}px;z-index:45;pointer-events:none;background:var(--paper);border-radius:12px;box-shadow:0 8px 24px rgb(0 0 0 / 0.25);transform-origin:50% 50%`;
  document.body.append(copy);
  row.style.visibility = "hidden";
  const a = reduced() ? null : copy.animate([{ transform: "scale(1)" }, { transform: `scale(${LIFT_SCALE})` }], { duration: 200, easing: "cubic-bezier(0.23, 1, 0.32, 1)", fill: "forwards" });
  return {
    move: (dx, dy) => { copy.style.translate = `${dx * 0.5}px ${dy * 0.5}px`; },
    drop: () => { const done = () => { copy.remove(); row.style.visibility = ""; }; if (!a) return done(); a.reverse(); a.onfinish = done; setTimeout(done, 260); },
    keep: () => { for (const x of copy.getAnimations?.() ?? []) x.cancel(); copy.remove(); Object.assign(copy.style, { translate: "", transform: "", boxShadow: "none", zIndex: "", borderRadius: "" }); return copy; },
  };
}

/** The menu item under a point, if the menu is open there. */
const itemAt = (x: number, y: number): HTMLElement | null => document.elementFromPoint(x, y)?.closest<HTMLElement>("[role=menuitem]") ?? null;

/** A list wears this: it arms the press of whichever row is touched, with four passive listeners on the scroller and nothing per row. */
export function pressRows(list: HTMLElement) {
  let row: HTMLElement | null = null, x0 = 0, y0 = 0, copy: HTMLElement | undefined, lifted: ReturnType<typeof liftCopy> | null = null, opened = false, hot: HTMLElement | null = null;
  const timers: number[] = [];
  let touch = false;
  const clear = () => { for (const t of timers) clearTimeout(t); timers.length = 0; };
  const block = (e: TouchEvent) => { if (opened && e.cancelable) e.preventDefault(); };
  const unarm = () => list.removeEventListener("touchmove", block);
  const heat = (el: HTMLElement | null) => { if (hot === el) return; hot?.removeAttribute("data-hot"); hot = el; hot?.setAttribute("data-hot", ""); };
  /** The press ends without opening: the lifted row settles back and the staged card goes. */
  const cancel = () => { clear(); unarm(); lifted?.drop(); lifted = null; pressState.stage = null; copy = undefined; row = null; };
  function down(e: PointerEvent) {
    touch = e.pointerType === "touch";
    if (!touch || !e.isPrimary) return;
    const r = (e.target as Element | null)?.closest<HTMLElement>("[data-press]");
    if (!r || !list.contains(r) || !bags.get(r)) return;
    cancel(); opened = false; heat(null);
    row = r; x0 = e.clientX; y0 = e.clientY;
    // Non-passive only while a row is held: after the menu opens the finger must slide onto an item, not scroll the list.
    list.addEventListener("touchmove", block, { passive: false });
    timers.push(window.setTimeout(() => { if (row) { copy = lookalike(row); void stagePress(row, copy); } }, PRIME_MS));
    timers.push(window.setTimeout(() => { if (row && copy) lifted = liftCopy(row, copy); }, LIFT_MS));
    timers.push(window.setTimeout(() => {
      if (!row) return;
      opened = true;
      const keep = lifted?.keep(); lifted = null; // the copy goes into the card (the row stays hidden until the press is put away)
      void openPress(row, true, x0, y0, keep ?? copy);
    }, MENU_MS));
  }
  function move(e: PointerEvent) {
    if (!row || !touch) return;
    if (opened) { heat(itemAt(e.clientX, e.clientY)); return; }
    const dx = e.clientX - x0, dy = e.clientY - y0;
    if (Math.hypot(dx, dy) > STILL) cancel(); else lifted?.move(dx, dy);
  }
  function up() {
    if (!row) return;
    if (opened) { const h = hot; heat(null); clear(); unarm(); row = null; if (h) h.click(); return; }
    cancel();
  }
  function menu(e: MouseEvent) {
    const r = (e.target as Element | null)?.closest<HTMLElement>("[data-press]");
    if (!r || !list.contains(r) || !bags.get(r)) return;
    // A touch's long press may also raise a context menu (Android): the hold above already owns it.
    if (touch) { e.preventDefault(); return; }
    e.preventDefault();
    void openPress(r, false, e.clientX, e.clientY);
  }
  function key(e: KeyboardEvent) {
    if (!(e.key === "ContextMenu" || (e.key === "F10" && e.shiftKey))) return;
    const r = (e.target as Element | null)?.closest<HTMLElement>("[data-press]");
    if (!r || !list.contains(r) || !bags.get(r)) return;
    e.preventDefault();
    const b = r.getBoundingClientRect();
    void openPress(r, false, b.left + 24, b.bottom);
  }
  list.addEventListener("pointerdown", down, { passive: true });
  list.addEventListener("pointermove", move, { passive: true });
  list.addEventListener("pointerup", up, { passive: true });
  list.addEventListener("pointercancel", up, { passive: true });
  list.addEventListener("contextmenu", menu);
  list.addEventListener("keydown", key);
  // The browser's own callout and text selection would take the hold: off on pressable rows only.
  const style = document.createElement("style");
  style.textContent = "[data-press]{-webkit-touch-callout:none}@media (pointer:coarse){[data-press]{-webkit-user-select:none;user-select:none}}[role=menuitem][data-hot]{background:var(--hover)}";
  if (!document.head.querySelector("style[data-press-style]")) { style.setAttribute("data-press-style", ""); document.head.append(style); }
  return { destroy() { cancel(); list.removeEventListener("pointerdown", down); list.removeEventListener("pointermove", move); list.removeEventListener("pointerup", up); list.removeEventListener("pointercancel", up); list.removeEventListener("contextmenu", menu); list.removeEventListener("keydown", key); } };
}

/** The preview registry: a kind of item and the component that draws it (`item` is its prop). */
import type { Component } from "svelte";
const previews = new Map<string, Component<{ item: never }>>();
/** Registers how an item of `kind` is previewed (a thread: its last messages; a note: its top). */
export function registerPreview<T>(kind: string, component: Component<{ item: T }>): void { previews.set(kind, component as unknown as Component<{ item: never }>); }
/** The component that previews `kind`, if one was registered. */
export const previewOf = (kind: string): Component<{ item: never }> | undefined => previews.get(kind);
