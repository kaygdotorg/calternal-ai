// Ported from the existing app (apps/web/src/rows/swipe-math.ts on origin/dev, #400) without changes to the numbers.
// The numbers behind a swipe row (#400): how far the row goes for a finger's distance, and how each
// button looks at that distance. Pure functions, no DOM, so the feel can be read (and tested) here.
//
// A "side" is the row's buttons on one edge: `n` buttons, whether a long swipe runs the outer one
// (`full`), and whether the row goes away when it does (`leaves`).

/** Which edge the buttons are on: 1 from the left (a swipe to the right), -1 from the right. */
export type Side = 1 | -1;
/** One side's buttons: how many (`n`), whether a long swipe runs the outer one (`full`) and whether the row goes away when it does (`leaves`). */
export type SideConf = { n: number; full: boolean; leaves: boolean };

/** Width in px of each button's cell in the tray. */
export const SLOT = 64;
/** How far in px the tray sits inside the row's edge. */
export const GAP = 4;
/** Width in px of an unstretched button pill. */
export const TILE = 56;
/** Past the buttons the row follows the finger at this share of its travel (the resistance). */
export const BAND = 0.55;
/** Release speed in px/ms above which the row opens (or shuts) whatever the distance travelled. */
export const FLICK = 0.3;

/** The side a signed finger distance belongs to (zero counts as the left). */
export const sideOf = (v: number): Side => (v >= 0 ? 1 : -1);
/** The tray's width in px for a side: its buttons times the cell width; 0 for a side with none. */
export const trayWidth = (c: SideConf) => c.n * SLOT;
/** Finger distance where a long swipe commits. */
export const fullAt = (c: SideConf, width: number) => Math.max(trayWidth(c) + 56, width * 0.55);

const smooth = (p: number) => p * p * (3 - 2 * p);

/** Finger distance → row offset: 1:1 to the buttons, then with resistance. */
export function band(raw: number, c: SideConf): number {
  const s = sideOf(raw);
  const a = Math.abs(raw);
  const w = trayWidth(c);
  if (!w) return s * Math.min(a * 0.12, 14);
  return s * (a <= w ? a : w + (a - w) * BAND);
}

/** How much of pill `i` is showing at row offset `a`, 0 to 1. */
const reveal = (i: number, a: number) => Math.min(1, Math.max(0, (a - (GAP + i * SLOT)) / 44));

/**
 * How far the outer pill has taken over, 0 to 1, from how far the row is pulled (row offset `a`).
 * Zero until the buttons are all out, one at the commit point: one continuous ramp.
 */
export function takeover(c: SideConf, width: number, a: number): number {
  const w = trayWidth(c);
  if (!w || a <= w) return 0;
  const raw = w + (a - w) / BAND;
  return smooth(Math.min(1, Math.max(0, (raw - w) / (fullAt(c, width) - w))));
}

/** One button's drawn state at a given pull: its width, how far it slides, its opacity, its scale and how solid its fill is (0 to 1). */
export type TileLook = { width: number; shift: number; opacity: number; scale: number; solid: number };

/** What pill `i` looks like with the row at offset `a`. The outer one stretches; the rest slide under. */
export function tileLook(i: number, s: Side, c: SideConf, width: number, a: number): TileLook {
  const p = takeover(c, width, a);
  const r = reveal(i, a);
  const w0 = TILE + p * (Math.max(TILE, a - 2 * GAP) - TILE);
  if (i === 0) return { width: w0, shift: 0, opacity: r, scale: 0.78 + 0.22 * r, solid: p };
  return { width: TILE, shift: s * (w0 - TILE), opacity: r * (1 - Math.min(1, p * 1.5)), scale: (0.78 + 0.22 * r) * (1 - 0.35 * p), solid: 0 };
}
