/**
 * Block ID markers (ADR 0056, rules B1 to B5 of the existing app): the one module that parses and writes `^id` markers. Nothing else in the editor, and nothing in a host, matches or
 * writes a marker itself. A marker is a `^` followed by 2 or more of `A-Za-z0-9-`, after whitespace or one of `[ , {`, and before whitespace, `] } , ) . ; :` or the end of the line. At the
 * end of a line it carries its leading spaces with it. New IDs are 16 lowercase Crockford characters; shorter legacy IDs stay valid and are never rewritten.
 *
 * Rules kept here (B1): only a task, or a block something links to, has an ID, so `markerFor` is called for those and for nothing else. The view side (hiding, typing next to a marker, new tasks,
 * pasted copies) is `ids-view.ts`, a lazy chunk loaded when a note holds a marker or a task. Never in a note's visible text: `visibleText` is what search, copy and accessibility use.
 * Not here yet (the server is the authority, B5): retiring IDs, aliases on join, and uniqueness across spaces. This module only guarantees uniqueness inside one note.
 */
import type { EditorState } from "@codemirror/state";

/** A marker found in a line: its full range (the separating space included at the end of a line) and the ID without the `^`. */
export interface Marker { from: number; to: number; id: string }

const CROCKFORD = "0123456789abcdefghjkmnpqrstvwxyz";
const TASK = /^\s*[-*+] \[[ xX]\]( |$)/;

function tokenEnd(text: string, end: number): boolean {
  const next = text[end];
  return next === undefined || /\s|[\]},).;:]/u.test(next);
}

/** Every marker in one line of text, as offsets into that line. Cost: one scan for `^`. */
export function markersIn(line: string): Marker[] {
  const out: Marker[] = [];
  for (let cursor = 0; ; ) {
    const caret = line.indexOf("^", cursor);
    if (caret < 0) break;
    const before = caret === 0 ? "" : line[caret - 1]!;
    if (before && !/\s|[[,{]/u.test(before)) { cursor = caret + 1; continue; }
    let end = caret + 1;
    while (end < line.length && /[A-Za-z0-9-]/.test(line[end]!)) end++;
    if (end - caret < 3 || !tokenEnd(line, end)) { cursor = caret + 1; continue; }
    // At the end of a line the single space that separates it from the text goes with it; any more spaces are the text's own (a person can type them).
    const from = /[ \t]/.test(before) && !line.slice(end).trim() ? caret - 1 : caret;
    out.push({ from, to: end, id: line.slice(caret + 1, end) });
    cursor = end;
  }
  return out;
}

/** The text a person sees: the same text with every marker removed. For search, copy and accessibility; the stored note keeps its markers. */
export function visibleText(text: string): string {
  return text.split("\n").map((l) => { const ms = markersIn(l); if (!ms.length) return l; let out = "", at = 0; for (const m of ms) { out += l.slice(at, m.from); at = m.to; } return out + l.slice(at); }).join("\n");
}

/** Every ID in the document, in order (duplicates included). One pass over the lines that contain a `^`. */
export function idsIn(state: EditorState): string[] {
  const out: string[] = [];
  const doc = state.doc;
  for (let i = 1; i <= doc.lines; i++) { const t = doc.line(i).text; if (t.includes("^")) for (const m of markersIn(t)) out.push(m.id); }
  return out;
}

/** A new 16-character ID that is not in `taken` (the collision check of ADR 0056). `rand` is injectable for tests; it defaults to the platform's random source. */
export function newId(taken: ReadonlySet<string>, rand: (n: number) => Uint8Array = (n) => crypto.getRandomValues(new Uint8Array(n))): string {
  for (;;) {
    const b = rand(16);
    let id = "";
    for (let i = 0; i < 16; i++) id += CROCKFORD[b[i]! & 31];
    if (!taken.has(id)) return id;
  }
}

/** True when this line is a task item (a list item that starts with a box). */
export const isTaskLine = (line: string): boolean => TASK.test(line);

/** The text to append at the end of a block's last line to give it `id`. */
export const markerFor = (id: string): string => ` ^${id}`;

/** The marker that ends this line, if one does (a marker anywhere else in the line belongs to something inside it, such as a link). */
export function endMarker(line: string): Marker | null {
  const ms = markersIn(line);
  const last = ms.at(-1);
  return last && last.to === line.length ? last : null;
}

/**
 * The change that gives the block whose last line starts at `lineFrom` an ID, and the ID. Returns the existing ID and no change when the line already ends in a marker.
 * This is what the first link to a block calls (B1), and what a new task calls.
 */
export function ensureId(state: EditorState, lineFrom: number): { id: string; change: { from: number; insert: string } | null } {
  const line = state.doc.lineAt(lineFrom);
  const have = endMarker(line.text);
  if (have) return { id: have.id, change: null };
  const id = newId(new Set(idsIn(state)));
  return { id, change: { from: line.to, insert: markerFor(id) } };
}
