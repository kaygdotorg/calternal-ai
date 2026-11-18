/**
 * The one view of "some text with a selection" that every text behaviour is written against: a string for a text field, CodeMirror's document for the block editor. A behaviour
 * (`commands.ts`) takes a `Doc` and a `Sel` and returns one `Change` and the selection after it, so the same code runs in both and a field and a note cannot drift apart.
 * CodeMirror's `Text` satisfies `Doc` as it is; `stringDoc` is the same thing over a string, built by walking its line breaks once.
 */
import type { Change } from "../transaction";

/** One line of a document. */
export interface Line { from: number; to: number; number: number; text: string }
/** Read-only text addressed by position and by line (lines start at 1). */
export interface Doc {
  /** The length in UTF-16 code units. */
  readonly length: number;
  /** The number of lines (at least 1). */
  readonly lines: number;
  /** The `n`th line (1-based); throws for a number outside the document. */
  line(n: number): Line;
  /** The line that contains a position. */
  lineAt(pos: number): Line;
  /** The text between two positions (to the end when `to` is omitted). */
  sliceString(from: number, to?: number): string;
}
/** A selection: where it started and where the caret is (they differ when text is selected). */
export interface Sel { anchor: number; head: number }
/** What a command does: one change, and the selection after it. */
export interface Edit { change: Change; selection: Sel }

/** A string as a `Doc`. */
export function stringDoc(text: string): Doc {
  const starts = [0];
  for (let i = text.indexOf("\n"); i >= 0; i = text.indexOf("\n", i + 1)) starts.push(i + 1);
  const mk = (n: number): Line => { const from = starts[n - 1]!, to = n < starts.length ? starts[n]! - 1 : text.length; return { from, to, number: n, text: text.slice(from, to) }; };
  return {
    length: text.length,
    lines: starts.length,
    line: (n) => { if (n < 1 || n > starts.length) throw new RangeError(`no line ${n}`); return mk(n); },
    lineAt: (pos) => { let lo = 0, hi = starts.length - 1; const p = Math.max(0, Math.min(text.length, pos)); while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid]! <= p) lo = mid; else hi = mid - 1; } return mk(lo + 1); },
    sliceString: (from, to = text.length) => text.slice(from, to),
  };
}
