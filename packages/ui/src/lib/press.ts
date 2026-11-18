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
}

/** A list wears this: it arms the press of whichever row is touched, with four passive listeners on the scroller and nothing per row. */
export function pressRows(list: HTMLElement) {
  let row: HTMLElement | null = null, x0 = 0, y0 = 0, copy: HTMLElement | undefined, lift: Animation | null = null, opened = false;
  const timers: number[] = [];
  let touch = false;
  const clear = () => { for (const t of timers) clearTimeout(t); timers.length = 0; };
  const settle = () => { if (lift) { lift.reverse(); lift = null; } };
  const end = (keep: boolean) => { clear(); if (!keep) settle(); if (!opened) copy = undefined; row = null; };
  function down(e: PointerEvent) {
    touch = e.pointerType === "touch";
    if (!touch || !e.isPrimary) return;
    const r = (e.target as Element | null)?.closest<HTMLElement>("[data-press]");
    if (!r || !list.contains(r) || !bags.get(r)) return;
    end(false); opened = false;
    row = r; x0 = e.clientX; y0 = e.clientY;
    timers.push(window.setTimeout(() => { if (row) copy = lookalike(row); }, PRIME_MS));
    timers.push(window.setTimeout(() => {
      if (!row || reduced()) return;
      lift = row.animate([{ scale: 1 }, { scale: LIFT_SCALE }], { duration: MENU_MS - LIFT_MS, easing: "linear", fill: "forwards" });
    }, LIFT_MS));
    timers.push(window.setTimeout(() => {
      if (!row) return;
      opened = true; settle();
      void openPress(row, true, x0, y0, copy);
    }, MENU_MS));
  }
  function move(e: PointerEvent) {
    if (!row || !touch) return;
    if (Math.hypot(e.clientX - x0, e.clientY - y0) > STILL) end(false);
  }
  const up = () => end(opened);
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
  style.textContent = "[data-press]{-webkit-touch-callout:none}@media (pointer:coarse){[data-press]{-webkit-user-select:none;user-select:none}}";
  if (!document.head.querySelector("style[data-press-style]")) { style.setAttribute("data-press-style", ""); document.head.append(style); }
  return { destroy() { end(false); list.removeEventListener("pointerdown", down); list.removeEventListener("pointermove", move); list.removeEventListener("pointerup", up); list.removeEventListener("pointercancel", up); list.removeEventListener("contextmenu", menu); list.removeEventListener("keydown", key); } };
}

/** The preview registry: a kind of item and the component that draws it (`item` is its prop). */
import type { Component } from "svelte";
const previews = new Map<string, Component<{ item: never }>>();
/** Registers how an item of `kind` is previewed (a thread: its last messages; a note: its top). */
export function registerPreview<T>(kind: string, component: Component<{ item: T }>): void { previews.set(kind, component as unknown as Component<{ item: never }>); }
/** The component that previews `kind`, if one was registered. */
export const previewOf = (kind: string): Component<{ item: never }> | undefined => previews.get(kind);
