/**
 * The canvas geometry (docs/ui-shell-status.md, "The canvas experiment"): the niri model. Columns sit side by side on an endless horizontal strip; a column holds windows stacked top to bottom that share its height by weight.
 * Pure functions, no DOM, so they can be tested and so the strip is positioned by arithmetic: nothing is measured while panning.
 */

/** The gap between columns and between stacked windows, and the strip's padding at its start (px). */
export const GAP = 12;
/** The narrowest a window may be (px) and the lowest a stacked window may be (share of the column's height). */
export const MIN_W = 280, MIN_SHARE = 0.18;

/** One window: an app's item (a thread, a note) with its own look. */
export interface Win {
  id: string;
  /** The app's kind in addresses (`threads`, `notes`). */
  kind: string;
  /** The item's id; with the kind it is the window's address (`/threads/<item>`). One window per address. */
  item: string;
  /** Its share of the column's height among the windows stacked in it. */
  weight: number;
  /** Frosted (the adaptive floating material) instead of the opaque wallpaper-derived colour. */
  frosted: boolean;
  /** The input lock: all input and scrolling stays in this window and the canvas never moves. */
  locked: boolean;
}

/** One column of the strip. */
export interface Col { id: string; w: number; wins: Win[] }

/** The left edge of column `i` on the strip. */
export const colLeft = (cols: readonly Col[], i: number): number => cols.slice(0, i).reduce((s, c) => s + c.w + GAP, GAP);

/** The strip's width up to the end of the last column. */
export const stripWidth = (cols: readonly Col[]): number => colLeft(cols, cols.length);

/** The furthest the strip may be scrolled: the last column may reach the left edge, so there is always room to put a new window beside it. */
export const maxScroll = (cols: readonly Col[], vw: number): number => Math.max(0, stripWidth(cols) - GAP - Math.min(vw, cols.at(-1)?.w ?? vw) - GAP);

/** The columns within `margin` px of the viewport [x, x + vw]: only these are mounted, so a window off screen is neither laid out nor painted. */
export function nearColumns(cols: readonly Col[], x: number, vw: number, margin: number): number[] {
  const out: number[] = [];
  let left = GAP;
  for (let i = 0; i < cols.length; i++) {
    const right = left + cols[i]!.w;
    if (right >= x - margin && left <= x + vw + margin) out.push(i);
    left = right + GAP;
  }
  return out;
}

/** The scroll position that brings column `i` fully into view with the least movement (centred when it cannot fit), clamped to the strip. */
export function scrollTo(cols: readonly Col[], i: number, x: number, vw: number): number {
  const left = colLeft(cols, i), w = cols[i]!.w;
  let next = x;
  if (w + 2 * GAP >= vw) next = left - (vw - w) / 2;
  else if (left - GAP < x) next = left - GAP;
  else if (left + w + GAP > x + vw) next = left + w + GAP - vw;
  return Math.min(Math.max(0, next), maxScroll(cols, vw));
}

/** The heights (px) of the windows of a column of height `h`, from their weights (the gaps between them taken out first). */
export function heightsOf(wins: readonly Win[], h: number): number[] {
  const room = h - GAP * (wins.length - 1), sum = wins.reduce((s, w) => s + w.weight, 0) || 1;
  return wins.map((w) => (room * w.weight) / sum);
}

/** Moves the boundary below window `i` of a column by `dy` px: window `i` grows by it and window `i + 1` shrinks (neither below `MIN_SHARE`). Returns new weights, or the old ones when it cannot move. */
export function dragBoundary(wins: readonly Win[], i: number, dy: number, h: number): number[] {
  const ws = wins.map((w) => w.weight), sum = ws.reduce((a, b) => a + b, 0), room = h - GAP * (wins.length - 1);
  const d = (dy / room) * sum;
  const min = MIN_SHARE * sum;
  const a = ws[i]! + d, b = ws[i + 1]! - d;
  if (a < min || b < min) return ws;
  ws[i] = a; ws[i + 1] = b;
  return ws;
}
