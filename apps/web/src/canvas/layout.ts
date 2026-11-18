/**
 * The canvas geometry (docs/ui-shell-status.md, "The rules"): niri's model. Columns sit side by side on an endless horizontal strip; a column holds windows stacked top to bottom that share its height by weight. Sizes come from
 * the layout: a column's width is a proportion of the area that takes the gaps and the inset into account (niri's `proportion`), so 1/n columns fit exactly. The windows are middle cards: they touch the top and bottom of the area, keep the card's inset
 * from its sides (none on a phone) and the same gap between each other. Pure functions, no DOM: the strip is positioned by arithmetic and nothing is measured while panning.
 */

/** The gap between columns and between stacked windows (px), the same as the card's inset from the sidebars. */
export const GAP = 8;
/** The area's inset at its sides, as today's middle card has from the sidebars: `GAP` on a wide screen, none on a phone (the card is edge to edge there). */
export const edgeOf = (vw: number): number => (vw < NARROW ? 0 : GAP);
/** The width presets a column cycles through (`switch-preset-column-width`): 1/3, 1/2, 2/3 and the full width. */
export const PRESETS = [1 / 3, 1 / 2, 2 / 3, 1] as const;
/** The narrowest a column may be dragged to, as a proportion, and the lowest a stacked window may be (share of its column's height). */
export const MIN_P = 0.2, MIN_SHARE = 0.18;
/** An edge drag within this many px of a preset snaps to it. */
export const SNAP = 12;
/** Below this canvas width every column is the full width (a phone). */
export const NARROW = 760;

/** The three looks of a window (docs/ui-shell-status.md, "The look of a window"). */
export type Look = "card" | "clear" | "solid";

/** One window: an app's item (a thread, a note) with its own look. */
export interface Win {
  id: string;
  /** The app's kind in addresses (`threads`, `notes`). */
  kind: string;
  /** The item's id; with the kind it is the window's address (`/threads/<item>`). One window per address. */
  item: string;
  /** Its share of the column's height among the windows stacked in it. */
  weight: number;
  /** How the window looks: the middle card as it is today (default), transparent (only a tint over the blurred, veiled background behind it) or solid (an opaque surface in the wallpaper's hue). */
  look: Look;
  /** The input lock: all input and scrolling stays in this window and the canvas never moves. */
  locked: boolean;
}

/** One column of the strip. `p` is its width as a proportion of the canvas, or null while the person has not sized it (the default rule applies: full width alone, half when there are two or more). */
export interface Col { id: string; p: number | null; wins: Win[] }

/** The proportion a column has: its own, else the default (the whole canvas for a single column, half each otherwise; always the whole canvas on a phone). */
export const proportionOf = (c: Col, n: number, vw: number): number => (vw < NARROW ? 1 : c.p ?? (n === 1 ? 1 : 0.5));

/** A column's width in px: `p x (vw - 2 edge + GAP) - GAP`, so that 1/p columns, their gaps and the two insets fill `vw` exactly; the full width is `vw - 2 edge` (the card). */
export const widthOf = (p: number, vw: number): number => Math.max(1, p * (vw - 2 * edgeOf(vw) + GAP) - GAP);

/** The widths of all columns. */
export const widthsOf = (cols: readonly Col[], vw: number): number[] => cols.map((c) => widthOf(proportionOf(c, cols.length, vw), vw));

/** The left edge of column `i`, given the widths. The strip starts one inset in. */
export const leftOf = (widths: readonly number[], i: number, vw: number): number => widths.slice(0, i).reduce((s, w) => s + w + GAP, edgeOf(vw));

/** The strip's width through the last column and the inset after it. */
export const stripWidth = (widths: readonly number[], vw: number): number => widths.reduce((s, w) => s + w + GAP, edgeOf(vw)) - GAP + edgeOf(vw);

/** The offset that centres a strip narrower than the area (niri's `always-center-single-column`, for any strip that fits); 0 when it does not fit. */
export const centring = (widths: readonly number[], vw: number): number => Math.max(0, (vw - stripWidth(widths, vw)) / 2);

/** The furthest the strip may be scrolled: to the end of the last column and its inset. */
export const maxScroll = (widths: readonly number[], vw: number): number => Math.max(0, stripWidth(widths, vw) - vw);

/** The columns within `margin` px of the viewport [x, x + vw]: only these are mounted, so a window off screen is neither laid out nor painted. */
export function nearColumns(widths: readonly number[], x: number, vw: number, margin: number, pad = 0): number[] {
  const out: number[] = [];
  let left = edgeOf(vw) + pad;
  for (let i = 0; i < widths.length; i++) {
    const right = left + widths[i]!;
    if (right >= x - margin && left <= x + vw + margin) out.push(i);
    left = right + GAP;
  }
  return out;
}

/** The scroll position that brings column `i` fully into view with the least movement (centred when it is wider than the canvas), clamped to the strip. */
export function scrollTo(widths: readonly number[], i: number, x: number, vw: number): number {
  const left = leftOf(widths, i, vw), w = widths[i]!, e = edgeOf(vw);
  let next = x;
  if (w + 2 * e >= vw) next = left - (vw - w) / 2;
  else if (left - e < x) next = left - e;
  else if (left + w + e > x + vw) next = left + w + e - vw;
  return Math.min(Math.max(0, next), maxScroll(widths, vw));
}

/** The next width preset after `p` (cycling; a size that is not a preset goes to the first preset above it). */
export function nextPreset(p: number, back = false): number {
  const list = [...PRESETS];
  const at = list.findIndex((x) => Math.abs(x - p) < 0.01);
  if (at >= 0) return list[(at + (back ? list.length - 1 : 1)) % list.length]!;
  return (back ? [...list].reverse().find((x) => x < p) : list.find((x) => x > p)) ?? list[0]!;
}

/** The proportion for a width dragged to `w` px, snapped to a preset within `SNAP` px, clamped to [MIN_P, 1]. */
export function proportionFor(w: number, vw: number): number {
  const p = Math.min(1, Math.max(MIN_P, (w + GAP) / (vw - 2 * edgeOf(vw) + GAP)));
  const snap = PRESETS.find((x) => Math.abs(widthOf(x, vw) - w) <= SNAP);
  return snap ?? p;
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
