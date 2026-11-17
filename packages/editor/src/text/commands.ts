/**
 * Text commands, written once. Each takes the document and the selection and returns the one change it makes and the selection after it (null when there is nothing to do), so a text field and
 * the block editor run the same code and behave the same: move, copy and delete lines, select a line, indent, collapse a selection. The key bindings are in `keys.ts`. The lines a command works
 * on are the ones the selection touches, except that a selection ending at the very start of a later line does not include that line (the usual rule). Cost: a few line reads and one slice.
 */
import type { Doc, Edit, Sel } from "./doc";

/** Two spaces: the indent a list item uses in Markdown. */
export const INDENT = "  ";

const lo = (s: Sel) => Math.min(s.anchor, s.head);
const hi = (s: Sel) => Math.max(s.anchor, s.head);
function span(doc: Doc, s: Sel): { first: number; last: number } {
  const a = lo(s), b = hi(s);
  const first = doc.lineAt(a).number;
  let last = doc.lineAt(b).number;
  if (b > a && b === doc.lineAt(b).from && last > first) last--;
  return { first, last };
}
const shift = (s: Sel, by: number): Sel => ({ anchor: s.anchor + by, head: s.head + by });

/** Moves the selected lines up or down one line; the selection goes with them. Null at the first or last line. */
export function moveLine(doc: Doc, s: Sel, up: boolean): Edit | null {
  const { first, last } = span(doc, s);
  const a = doc.line(first), z = doc.line(last);
  const block = doc.sliceString(a.from, z.to);
  if (up) {
    if (first === 1) return null;
    const p = doc.line(first - 1);
    return { change: { from: p.from, to: z.to, insert: `${block}\n${p.text}` }, selection: shift(s, -(p.text.length + 1)) };
  }
  if (last === doc.lines) return null;
  const n = doc.line(last + 1);
  const to = shift(s, n.text.length + 1);
  return { change: { from: a.from, to: n.to, insert: `${n.text}\n${block}` }, selection: { anchor: Math.min(to.anchor, doc.length), head: Math.min(to.head, doc.length) } }; // (a selection ending at the next line's start would otherwise point past the end)
}

/** Copies the selected lines above or below themselves. Going up the selection stays where it is (on the copy above); going down it moves to the copy. */
export function copyLine(doc: Doc, s: Sel, up: boolean): Edit {
  const { first, last } = span(doc, s);
  const a = doc.line(first), z = doc.line(last);
  const block = doc.sliceString(a.from, z.to);
  if (up) return { change: { from: a.from, to: a.from, insert: `${block}\n` }, selection: s }; // the selection stays where it is, which is now on the copy above
  return { change: { from: z.to, to: z.to, insert: `\n${block}` }, selection: shift(s, block.length + 1) };
}

/** Deletes the selected lines with one line break; the caret goes to the start of what follows (or the end of what precedes). */
export function deleteLine(doc: Doc, s: Sel): Edit {
  const { first, last } = span(doc, s);
  const a = doc.line(first), z = doc.line(last);
  let from = a.from, to = z.to;
  if (last < doc.lines) to = z.to + 1;
  else if (first > 1) from = a.from - 1;
  const at = Math.min(from, Math.max(0, doc.length - (to - from)));
  return { change: { from, to, insert: "" }, selection: { anchor: at, head: at } };
}

/** Selects the whole line the caret is on, with its line break. */
export function selectLine(doc: Doc, s: Sel): Edit | null {
  const { first, last } = span(doc, s);
  const a = doc.line(first), z = doc.line(last);
  return { change: { from: 0, to: 0, insert: "" }, selection: { anchor: a.from, head: Math.min(doc.length, z.to + 1) } };
}

/** Indents (more) or outdents (less) every selected line by one indent. */
export function indent(doc: Doc, s: Sel, more: boolean): Edit | null {
  const { first, last } = span(doc, s);
  const a = doc.line(first), z = doc.line(last);
  const lines: string[] = [], deltas: number[] = [];
  for (let n = first; n <= last; n++) {
    const t = doc.line(n).text;
    let next = t;
    if (more) next = INDENT + t;
    else { const m = /^( {1,2}|\t)/.exec(t); if (m) next = t.slice(m[0].length); }
    deltas.push(next.length - t.length);
    lines.push(next);
  }
  const total = deltas.reduce((x, d) => x + d, 0);
  if (!total) return null;
  // A position moves by the changes of the lines before it, and by its own line's change unless it sits at the line's start (it never moves back past it).
  const map = (p: number): number => {
    const l = doc.lineAt(p);
    if (l.number > last) return p + total;
    let add = 0;
    for (let n = first; n < Math.min(l.number, last + 1); n++) add += deltas[n - first]!;
    if (l.number >= first && p > l.from) add += Math.max(deltas[l.number - first]!, -(p - l.from));
    return p + add;
  };
  return { change: { from: a.from, to: z.to, insert: lines.join("\n") }, selection: { anchor: map(s.anchor), head: map(s.head) } };
}

/** Escape with a selection collapses it to the caret. Null when nothing is selected. */
export function collapse(_doc: Doc, s: Sel): Edit | null {
  if (s.anchor === s.head) return null;
  return { change: { from: 0, to: 0, insert: "" }, selection: { anchor: s.head, head: s.head } };
}

/** The commands by name, for the key table. */
export const COMMANDS = {
  moveLineUp: (d: Doc, s: Sel) => moveLine(d, s, true),
  moveLineDown: (d: Doc, s: Sel) => moveLine(d, s, false),
  copyLineUp: (d: Doc, s: Sel) => copyLine(d, s, true),
  copyLineDown: (d: Doc, s: Sel) => copyLine(d, s, false),
  deleteLine,
  selectLine,
  indentMore: (d: Doc, s: Sel) => indent(d, s, true),
  indentLess: (d: Doc, s: Sel) => indent(d, s, false),
  collapse,
} as const;
/** A command's name. */
export type CommandId = keyof typeof COMMANDS;
