// Swipe rows for a whole list with a handful of listeners. Ported from the existing app's `rows/swipe-rows.ts` (#400): the list's scroller wears `use:swipeRows`; every row inside is a
// `SwipeRow` that registers itself here. A list of any length has the same few listeners, and a row gets its gesture state only when someone first touches it (row.ts).
// (The existing app loads the engine as a lazy chunk; here it is imported directly, 6 KiB, because the Threads sidebar is on the first screen of the app.)
import { createRow, type Row, type RowHandle } from "./row";

const handles = new WeakMap<HTMLElement, RowHandle>();
const rows = new WeakMap<HTMLElement, Row>();

/** A row says how to reach its actions. Returns what to call when it leaves the page. */
export function registerRow(node: HTMLElement, handle: RowHandle): () => void {
  handles.set(node, handle);
  return () => {
    handles.delete(node);
    rows.get(node)?.destroy();
    rows.delete(node);
  };
}

const rowAt = (target: EventTarget | null): { node: HTMLElement; row: Row } | null => {
  const node = (target as Element | null)?.closest?.<HTMLElement>(".ra");
  const handle = node && handles.get(node);
  if (!node || !handle) return null;
  let row = rows.get(node);
  if (!row) rows.set(node, (row = createRow(node, handle)));
  return { node, row };
};

/** Svelte action for the list's scroller (or any ancestor of its rows). */
export function swipeRows(list: HTMLElement) {
  // The row a touch began on gets all of that touch's events, wherever they land.
  let active: Row | null = null;
  const tstart = (e: TouchEvent): void => {
    const hit = rowAt(e.target);
    if (active && active !== hit?.row) active.tcancel();
    active = hit?.row ?? null;
    active?.tstart(e);
  };
  const tmove = (e: TouchEvent) => active?.tmove(e);
  const tend = (e: TouchEvent) => { active?.tend(e); active = null; };
  const tcancel = () => { active?.tcancel(); active = null; };
  const wheel = (e: WheelEvent) => rowAt(e.target)?.row.wheel(e);
  const key = (e: KeyboardEvent) => e.key === "Escape" && rowAt(e.target)?.row.key(e);
  // Non-passive: a claimed swipe has to stop the page's own scroll (iOS, WebKit 232545).
  list.addEventListener("touchstart", tstart, { passive: false });
  list.addEventListener("touchmove", tmove, { passive: false });
  list.addEventListener("touchend", tend, { passive: false });
  list.addEventListener("touchcancel", tcancel);
  list.addEventListener("wheel", wheel, { passive: false });
  list.addEventListener("keydown", key);
  return {
    destroy() {
      list.removeEventListener("touchstart", tstart);
      list.removeEventListener("touchmove", tmove);
      list.removeEventListener("touchend", tend);
      list.removeEventListener("touchcancel", tcancel);
      list.removeEventListener("wheel", wheel);
      list.removeEventListener("keydown", key);
    },
  };
}
