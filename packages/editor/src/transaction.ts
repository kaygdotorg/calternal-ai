/**
 * A change to text: replace `[from, to)` with `insert`. Positions count UTF-16 code units, the unit the server uses; a change never splits a character (a surrogate pair or a
 * combining sequence start), so `snap` moves a position out of the middle of one. Pure functions, no DOM.
 */
import type { Origin } from "./origins";

/** Replace `[from, to)` with `insert`, positions in UTF-16 code units (the server's unit). */
export interface Change { from: number; to: number; insert: string }
/** A `Change` that knows where it came from (typing, a command, remote, replay), which decides whether it joins an undo step or waits while composing. */
export interface Transaction extends Change { origin: Origin }

const isLow = (c: number) => c >= 0xdc00 && c <= 0xdfff;
const isHigh = (c: number) => c >= 0xd800 && c <= 0xdbff;

/** Moves `pos` out of the middle of a surrogate pair (to the start of the pair). */
export function snap(text: string, pos: number): number {
  if (pos > 0 && pos < text.length && isLow(text.charCodeAt(pos)) && isHigh(text.charCodeAt(pos - 1))) return pos - 1;
  return Math.max(0, Math.min(text.length, pos));
}

/** Applies a change to text; throws on a range outside the text. */
export function apply(text: string, c: Change): string {
  if (c.from < 0 || c.to < c.from || c.to > text.length) throw new RangeError(`change ${c.from}-${c.to} is outside text of length ${text.length}`);
  return text.slice(0, c.from) + c.insert + text.slice(c.to);
}

/** The change that undoes `c` when applied to the result of applying `c` to `text`. */
export function invert(text: string, c: Change): Change {
  return { from: c.from, to: c.from + c.insert.length, insert: text.slice(c.from, c.to) };
}

/** The smallest single change turning `a` into `b` (common prefix and suffix, never splitting a surrogate pair). Cost: one pass over the shorter text. */
export function diff(a: string, b: string): Change | null {
  if (a === b) return null;
  let s = 0;
  const max = Math.min(a.length, b.length);
  while (s < max && a.charCodeAt(s) === b.charCodeAt(s)) s++;
  s = snap(a, s);
  let ea = a.length, eb = b.length;
  while (ea > s && eb > s && a.charCodeAt(ea - 1) === b.charCodeAt(eb - 1)) { ea--; eb--; }
  // Do not leave a lone low surrogate at the start of the tail.
  if (ea < a.length && eb < b.length && isLow(a.charCodeAt(ea)) ) { ea++; eb++; }
  return { from: s, to: ea, insert: b.slice(s, eb) };
}

/** Maps a position through a change (a position inside the replaced range goes to its start; after it, shifts by the length difference). */
export function mapPos(pos: number, c: Change): number {
  if (pos <= c.from) return pos;
  if (pos >= c.to) return pos + c.insert.length - (c.to - c.from);
  return c.from;
}
