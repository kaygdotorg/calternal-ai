/**
 * The rules of the sidebar pull: a horizontal swipe over the middle card brings a closed sidebar into view, following the finger one to one, and the opposite swipe closes it. One set of rules for a phone
 * (the card slides aside), and a touch screen of any width (one finger; a trackpad's horizontal wheel never pulls, owner decision 2026-11-18). Pure: no DOM state is kept here; the shell moves the pieces.
 *
 * What a swipe never takes (docs/ui-shell.md, "Sidebar pull"):
 *  - a swipe that starts on something that scrolls sideways and can still scroll that way (a code block, a table, a text field wider than its box): that element keeps it, for the whole swipe;
 *  - a swipe that starts on a row's own swipe actions (`.ra`), a dialog or sheet, or anything marked `data-no-pull`;
 *  - a touch while text is selected (the handles drag the selection, not the card);
 *  - a mostly vertical movement (it must be 1.5 times more across than down, and 10 px across, before it counts);
 *  - a mouse (a mouse drag selects text) or a trackpad's wheel: only touch pulls.
 */
export type Side = "left" | "right";

/** What a swipe would do: bring `side` into view (`opening`) or put it away. */
export interface Target { side: Side; opening: boolean }

/** `dir` is +1 when the finger moves right, -1 when it moves left. Swiping right closes an open right sidebar, else opens the left one; swiping left closes an open left one, else opens the right (when the app has one). Null: nothing to do. */
export function pickTarget(dir: 1 | -1, s: { leftOpen: boolean; rightOpen: boolean; hasRight: boolean }): Target | null {
  if (dir > 0) return s.rightOpen ? { side: "right", opening: false } : s.leftOpen ? null : { side: "left", opening: true };
  return s.leftOpen ? { side: "left", opening: false } : s.rightOpen || !s.hasRight ? null : { side: "right", opening: true };
}

/** How many px of the sidebar a finger displacement `dx` (right positive) shows, given where it began: `from` is 0 when opening and the sidebar's width when closing. */
export function visible(t: Target, from: number, dx: number, width: number): number {
  const along = t.side === "left" ? dx : -dx;
  return Math.max(0, Math.min(width, from + along));
}

/** Release: open when the speed along the opening direction (px per ms) is a flick that way, closed when it is a flick the other way, else by whether more than half is showing. */
export function settleOpen(vis: number, width: number, velocityAlong: number, flick = 0.4): boolean {
  return velocityAlong > flick ? true : velocityAlong < -flick ? false : vis > width / 2;
}

/** Whether the pointer went down on something a pull must leave alone (see the rules above). `dir` is the swipe's direction once known, else 0 (only the direction-free rules apply). */
export function leavesAlone(target: Element | null, dir: 0 | 1 | -1, selecting: boolean): boolean {
  if (selecting) return true;
  if (!target) return false;
  if (target.closest(".ra, [role=dialog], [data-no-pull]")) return true;
  return dir !== 0 && scrollsSideways(target, dir);
}

/** The nearest ancestor (up to the middle card) that scrolls sideways and still can in the direction the content would move (`dir` +1: the finger moves right, so the content scrolls toward its start). */
export function scrollsSideways(from: Element, dir: 1 | -1): boolean {
  for (let e: Element | null = from; e && !e.matches(".mid, .shell, body"); e = e.parentElement) {
    if (e.scrollWidth <= e.clientWidth + 1) continue;
    if (!/(auto|scroll)/.test(getComputedStyle(e).overflowX)) continue;
    const max = e.scrollWidth - e.clientWidth;
    if (dir > 0 ? e.scrollLeft > 0 : e.scrollLeft < max - 1) return true;
  }
  return false;
}

/** A CSS time (`340ms`, `0.19s`, a bare number is ms) in milliseconds; `fallback` when it is not one. A computed custom property keeps the unit it was written in, so both forms occur. */
export function parseMs(value: string, fallback: number): number {
  const m = /^\s*(-?[\d.]+)\s*(ms|s)?\s*$/.exec(value);
  if (!m) return fallback;
  const n = parseFloat(m[1]!);
  return Number.isFinite(n) ? (m[2] === "s" ? n * 1000 : n) : fallback;
}
