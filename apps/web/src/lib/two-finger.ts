// Two fingers swiped down over a list start selecting, and the rows they cross are picked (#470),
// the way the system's own Notes and Mail do it. A Svelte action on the list's scroller.
//
// - Two fingers down, moving down more than sideways by 12 px: select mode begins. From there the
//   rows between where the fingers started and where they are now are picked; drag back up and the
//   ones they leave are let go again. If select mode was already on, dragging up unpicks the rows
//   crossed. Rows picked before stay picked.
// - The fingers' middle decides the row. The rows' positions are read once when the gesture is
//   claimed, never per move, and the only per-frame work is the set of picked ids.
// - Near the top or bottom of the list the page scrolls on its own so a long list can be crossed.
// - A scroll with two fingers is this gesture's, not the list's: every two-finger move is
//   cancelled from the first one (a move that was left to scroll can never be taken back on iOS,
//   WebKit 232545). One finger scrolls as always. Pinch can't zoom the list for the same reason.
//
// Rows say who they are with `data-select-id`.
// Ported from the existing app (rows/two-finger.ts). Changes: the list is scrolled by its enclosing `.scroll` (the shell's sidebar scroller), not by the node the action sits on, and there is no open swipe row to shut
// or a document scroll to follow here.
import { beginSelect, selection, setPicked } from "./select.svelte";

export type TwoFingerConfig = { kind: string };

const CLAIM = 12;
const EDGE = 56;

type Row = { id: string; top: number; bottom: number };

export function twoFinger(host: HTMLElement, config: TwoFingerConfig) {
  const node: HTMLElement = host.closest<HTMLElement>(".scroll") ?? host;
  let cfg = config;
  let g: { y0: number; x0: number; claimed: boolean; base: Set<string>; rows: Row[]; y: number; scroll0: number } | null = null;
  let raf = 0;

  const mid = (e: TouchEvent) => ({
    x: (e.touches[0].clientX + e.touches[1].clientX) / 2,
    y: (e.touches[0].clientY + e.touches[1].clientY) / 2,
  });

  /** Where a screen y lies in the list's own coordinates (scrolling included). */
  const inList = (y: number) => y - node.getBoundingClientRect().top + node.scrollTop;

  function measureRows(): Row[] {
    const box = node.getBoundingClientRect();
    return [...node.querySelectorAll<HTMLElement>("[data-select-id]")].map((el) => {
      const r = el.getBoundingClientRect();
      return { id: el.dataset.selectId!, top: r.top - box.top + node.scrollTop, bottom: r.bottom - box.top + node.scrollTop };
    });
  }

  function apply() {
    if (!g?.claimed) return;
    const y = inList(g.y);
    const a = Math.min(g.y0, y);
    const b = Math.max(g.y0, y);
    const crossed = g.rows.filter((r) => r.bottom > a && r.top < b).map((r) => r.id);
    const down = y >= g.y0;
    const next = new Set(g.base);
    // Down picks what is crossed; up unpicks it, but only rows that were picked before this gesture.
    if (down) for (const id of crossed) next.add(id);
    else for (const id of crossed) next.delete(id);
    setPicked(next);
  }

  /** Near an edge, the list moves under the fingers. */
  function edgeScroll() {
    raf = 0;
    if (!g?.claimed) return;
    const box = node.getBoundingClientRect();
    const bottom = box.bottom;
    const top = box.top;
    const toBottom = g.y - (bottom - EDGE);
    const toTop = top + EDGE - g.y;
    const v = toBottom > 0 ? Math.min(14, toBottom / 4) : toTop > 0 ? -Math.min(14, toTop / 4) : 0;
    if (v) {
      node.scrollTop += v;
      apply();
      raf = requestAnimationFrame(edgeScroll);
    }
  }

  const start = (e: TouchEvent) => {
    if (e.touches.length !== 2) return void (g = null);
    const m = mid(e);
    g = { y0: inList(m.y), x0: m.x, claimed: false, base: new Set(selection.on && selection.kind === cfg.kind ? selection.ids : []), rows: [], y: m.y, scroll0: node.scrollTop };
  };

  const move = (e: TouchEvent) => {
    if (e.touches.length !== 2) return;
    if (!g) return start(e);
    // Two fingers belong to this list from the first move.
    if (e.cancelable) e.preventDefault();
    const m = mid(e);
    g.y = m.y;
    if (!g.claimed) {
      const dy = inList(m.y) - g.y0;
      const dx = m.x - g.x0;
      const on = selection.on && selection.kind === cfg.kind;
      const goes = on ? Math.abs(dy) >= CLAIM : dy >= CLAIM && dy > 1.2 * Math.abs(dx);
      if (!goes) return;
      g.claimed = true;
      g.rows = measureRows();
      if (!on) beginSelect(cfg.kind);
    }
    apply();
    raf ||= requestAnimationFrame(edgeScroll);
  };

  const end = (e: TouchEvent) => {
    if (e.touches.length >= 2) return;
    // A gesture that selected something is not also a tap.
    if (g?.claimed && e.cancelable) e.preventDefault();
    g = null;
    cancelAnimationFrame(raf);
    raf = 0;
  };

  // (Pinch never zooms the page anywhere: no-page-zoom.ts, so the list needs no handler of its own.)
  host.addEventListener("touchstart", start, { passive: true });
  host.addEventListener("touchmove", move, { passive: false });
  host.addEventListener("touchend", end, { passive: false });
  host.addEventListener("touchcancel", end);
  return {
    update(next: TwoFingerConfig) {
      cfg = next;
    },
    destroy() {
      cancelAnimationFrame(raf);
      host.removeEventListener("touchstart", start);
      host.removeEventListener("touchmove", move);
      host.removeEventListener("touchend", end);
      host.removeEventListener("touchcancel", end);
    },
  };
}
