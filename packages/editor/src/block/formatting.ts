/**
 * What the bottom bar and the formatting shortcuts do to the text, as plain edits of the Markdown (ported from the existing app's `editing.svelte.ts` and `cm-extras.ts`, apps/web/src/lib): marks around
 * words (`**`, `*`, `~~`, a backtick, `==`), the block's kind (a line prefix), and the whole-block actions (duplicate, move, delete). There is no second representation of the note: every function
 * here reads the editor's own text and returns or dispatches ordinary changes, so a change saves, syncs and undoes exactly like typing does. Cost: reads of the lines around the selection, never
 * the whole document; nothing here runs per key (the host calls `readSelection` after a selection change, and debounced after a text change).
 */
import { EditorSelection, Prec, type EditorState, type Extension, type Range, type TransactionSpec } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, keymap, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { blockAt } from "./blocks";
import { frontmatterEnd } from "./frontmatter";
import { origin } from "./origin";

/** The kinds of block the bar names (Title, Subtitle, Heading, Body, List, Numbered, To-do, Quote, Code, Divider). */
export type BlockType = "h1" | "h2" | "h3" | "p" | "li" | "ol" | "todo" | "quote" | "code" | "divider";
/** A range of the text, from the start of a block to its end. */
export interface Span { from: number; to: number }
/** The marks the bar toggles. */
export type MarkKind = "bold" | "italic" | "strike" | "code" | "highlight";
/** Which marks surround the selected words. */
export type Marks = Record<MarkKind, boolean>;

/** The characters each mark writes around words. Highlight is `==words==` (Obsidian, Bear, iA Writer): plain text in any other Markdown app. */
export const MARKS: Record<MarkKind, string> = { bold: "**", italic: "*", strike: "~~", code: "`", highlight: "==" };

const LIST = /^(\s*)([-*+]|\d+[.)])( +)(\[([ xX/-])\] ?)?/;
const LEAD = /^(\s*)(#{1,6} |> ?|([-*+]|\d+[.)]) +(\[[ xX/-]\] ?)?)?/;
/** The line prefix each block type writes. */
const PREFIX: Record<BlockType, string> = { h1: "# ", h2: "## ", h3: "### ", p: "", li: "- ", ol: "1. ", todo: "- [ ] ", quote: "> ", code: "", divider: "" };

/** The type of block a line of text is. */
export function typeOfLine(text: string): BlockType {
  const t = text.trimStart();
  if (t.startsWith("### ")) return "h3";
  if (t.startsWith("## ")) return "h2";
  if (t.startsWith("# ")) return "h1";
  if (t.startsWith("> ")) return "quote";
  if (t.startsWith("```") || t.startsWith("~~~")) return "code";
  if (/^(---|\*\*\*|___)\s*$/.test(t)) return "divider";
  const m = LIST.exec(text);
  if (m) return m[4] ? "todo" : /\d/.test(m[2]!) ? "ol" : "li";
  return "p";
}

/** The blocks (outermost spans) that touch `[from, to]`, below the frontmatter. One syntax-tree lookup per line walked. */
export function spansIn(state: EditorState, from: number, to: number): Span[] {
  const out: Span[] = [];
  const body = frontmatterEnd(state);
  let pos = Math.max(from, body);
  while (pos <= to && pos <= state.doc.length) {
    const line = state.doc.lineAt(pos);
    const b = blockAt(state, line.from);
    if (b && !out.some((s) => s.from === b.from)) out.push({ from: b.from, to: b.to });
    pos = Math.max(line.to + 1, b ? b.to + 1 : 0);
    if (line.to >= state.doc.length) break;
  }
  return out;
}

/** Whether `mark` surrounds `[from, to]`. */
export function around(state: EditorState, from: number, to: number, mark: string): boolean {
  const d = state.doc;
  return d.sliceString(Math.max(0, from - mark.length), from) === mark && d.sliceString(to, to + mark.length) === mark;
}

/** Which marks surround the selected words. */
export function marksAround(state: EditorState, from: number, to: number): Marks {
  return {
    bold: around(state, from, to, "**"),
    italic: around(state, from, to, "*") && !around(state, from, to, "**"),
    strike: around(state, from, to, "~~"),
    code: around(state, from, to, "`"),
    highlight: around(state, from, to, "=="),
  };
}

/** A command's edit: the person's own change, undoable (`origin` command), the caret kept in sight. */
function command(view: EditorView, spec: TransactionSpec): void {
  view.dispatch({ ...spec, scrollIntoView: true, userEvent: "input.command", annotations: [origin.of("command")] });
  // Giving focus back only when it was lost: focusing an editor that has focus makes iOS think the input moved and its keyboard jumps.
  if (!view.hasFocus) view.focus();
}

/** Wraps the selected words in a mark, or takes it off when it is already there. With only a caret, starts the mark here (`**|**`), and again on the empty pair takes it back. */
export function toggleMark(view: EditorView, kind: MarkKind): void {
  const m = MARKS[kind];
  const { from, to } = view.state.selection.main;
  if (from === to) {
    const doc = view.state.doc;
    // Only an exact pair: the `*` of italic inside `**|**` is bold's, not an empty italic.
    const exact = doc.sliceString(from - m.length - 1, from - m.length) !== m[0];
    if (exact && doc.sliceString(from - m.length, from) === m && doc.sliceString(from, from + m.length) === m) command(view, { changes: { from: from - m.length, to: from + m.length }, selection: EditorSelection.cursor(from - m.length) });
    else command(view, { changes: { from, insert: m + m }, selection: EditorSelection.cursor(from + m.length) });
    return;
  }
  if (around(view.state, from, to, m)) command(view, { changes: [{ from: from - m.length, to: from }, { from: to, to: to + m.length }], selection: EditorSelection.range(from - m.length, to - m.length) });
  else command(view, { changes: [{ from, insert: m }, { from: to, insert: m }], selection: EditorSelection.range(from + m.length, to + m.length) });
}

/** The spans an action applies to: the selected blocks, or the line the caret is in. */
function targets(state: EditorState, blocks: Span[]): Span[] {
  if (blocks.length) return blocks;
  const line = state.doc.lineAt(state.selection.main.head);
  return [{ from: line.from, to: line.to }];
}

/** Turns the selected blocks (or the caret's line) into another type. */
export function setType(view: EditorView, blocks: Span[], type: BlockType): void {
  const state = view.state;
  const changes: { from: number; to: number; insert: string }[] = [];
  for (const span of targets(state, blocks)) {
    if (type === "code") {
      const text = state.doc.sliceString(span.from, span.to).replace(/^\s*([-*+]|\d+[.)])\s+(\[[ xX/-]\]\s+)?|^#{1,6}\s+|^>\s?/gm, "");
      changes.push({ from: span.from, to: span.to, insert: "```\n" + text + "\n```" });
      continue;
    }
    if (type === "divider") { changes.push({ from: span.from, to: span.to, insert: "---" }); continue; }
    for (let pos = span.from; pos <= span.to; ) {
      const line = state.doc.lineAt(pos);
      const lead = LEAD.exec(line.text)!;
      const keepIndent = type === "li" || type === "ol" || type === "todo" ? (lead[1] ?? "") : "";
      changes.push({ from: line.from, to: line.from + lead[0].length, insert: keepIndent + PREFIX[type] });
      if (line.to >= span.to) break;
      pos = line.to + 1;
    }
  }
  command(view, { changes });
}

/** Deletes the selected blocks (each with the line break after it). Returns how many, for the toast ("Block deleted"). */
export function deleteBlocks(view: EditorView, blocks: Span[]): number {
  if (!blocks.length) return 0;
  const state = view.state;
  const spans = [...blocks].sort((a, b) => b.from - a.from);
  const changes = spans.map((s) => ({ from: s.from, to: Math.min(state.doc.length, s.to + (s.to < state.doc.length ? 1 : 0)) }));
  const removed = changes.reduce((n, c) => n + (c.to - c.from), 0);
  command(view, { changes, selection: EditorSelection.cursor(Math.max(0, Math.min(spans.at(-1)!.from, state.doc.length - removed))) });
  return spans.length;
}

/**
 * Copies exactly the block (or the selected blocks) right under itself and puts the caret at the same spot in the copy. A list item copies onto the next line; anything else needs a blank line, or the copy
 * would run into the original as one paragraph. Returns false when there is no block.
 */
export function duplicateBlocks(view: EditorView, blocks: Span[]): boolean {
  const state = view.state;
  const head = state.selection.main.head;
  const spans = blocks.length ? blocks : spansIn(state, head, head);
  if (!spans.length) return false;
  const first = spans[0]!, last = spans.at(-1)!;
  const start = state.doc.lineAt(first.from).from; // from the line's start: a nested item's block begins after its indent, which the copy keeps
  const text = state.doc.sliceString(start, last.to);
  const gap = LIST.test(text) ? "\n" : "\n\n";
  const caret = blocks.length ? text.length : Math.min(Math.max(head - start, 0), text.length);
  command(view, { changes: { from: last.to, insert: gap + text }, selection: EditorSelection.cursor(last.to + gap.length + caret) });
  return true;
}

/** Moves the selected blocks one block up or down. Returns the moved span (the blocks stay selected), or null when there is nowhere to go. */
export function nudgeBlocks(view: EditorView, blocks: Span[], dir: -1 | 1): Span | null {
  const state = view.state;
  const spans = blocks.length ? blocks : spansIn(state, state.selection.main.head, state.selection.main.head);
  if (!spans.length) return null;
  const first = spans[0]!, last = spans.at(-1)!;
  const chunk = state.doc.sliceString(first.from, last.to);
  const body = frontmatterEnd(state);
  if (dir < 0) {
    const prevLine = state.doc.lineAt(first.from).number - 1;
    if (prevLine < 1) return null;
    const prev = state.doc.line(prevLine);
    if (prev.from < body) return null;
    const target = blockAt(state, prev.text.trim() ? prev.from : Math.max(body, prev.from - 1)) ?? { from: prev.from, to: prev.to };
    if (target.from < body) return null;
    const between = state.doc.sliceString(target.to, first.from);
    const prevText = state.doc.sliceString(target.from, target.to);
    command(view, { changes: { from: target.from, to: last.to, insert: chunk + between + prevText }, selection: EditorSelection.range(target.from, target.from + chunk.length) });
    return { from: target.from, to: target.from + chunk.length };
  }
  const nextLine = state.doc.lineAt(last.to).number + 1;
  if (nextLine > state.doc.lines) return null;
  let n = nextLine;
  while (n <= state.doc.lines && !state.doc.line(n).text.trim()) n++;
  if (n > state.doc.lines) return null;
  const next = blockAt(state, state.doc.line(n).from) ?? { from: state.doc.line(n).from, to: state.doc.line(n).to };
  const between = state.doc.sliceString(last.to, next.from);
  const nextText = state.doc.sliceString(next.from, next.to);
  const start = first.from + nextText.length + between.length;
  command(view, { changes: { from: first.from, to: next.to, insert: nextText + between + chunk }, selection: EditorSelection.range(start, start + chunk.length) });
  return { from: start, to: start + chunk.length };
}

/** The block's own span and the section under it (a heading takes everything up to the next heading of its level or above; a list item, its nested items). */
export function withChildren(state: EditorState, s: Span): Span {
  const first = state.doc.lineAt(s.from).text;
  const h = /^(#{1,6}) /.exec(first);
  const indent = /^(\s*)([-*+]|\d+[.)]) /.exec(first);
  let to = s.to;
  for (let n = state.doc.lineAt(s.to).number + 1; n <= state.doc.lines; n++) {
    const line = state.doc.line(n);
    if (h) { const hh = /^(#{1,6}) /.exec(line.text); if (hh && hh[1]!.length <= h[1]!.length) break; }
    else if (indent) { const ind = /^(\s*)/.exec(line.text)![1]!.length; if (line.text.trim() === "" || ind <= indent[1]!.length) break; }
    else break;
    to = line.to;
  }
  return { from: s.from, to };
}

// ---- Highlighted words ----

// `==Highlighted words==` are drawn with a marker; the `==` hide like other marks, except in source mode, where every character shows.
const hl = Decoration.mark({ class: "cm-hl" });
const hide = Decoration.replace({});
const HL = /==(?=[^\s=])([^=\n]*?[^\s=])==/g;

function highlightDecos(view: EditorView): DecorationSet {
  const source = view.dom.dataset.mode === "source";
  const out: Range<Decoration>[] = [];
  const body = frontmatterEnd(view.state);
  for (const { from, to } of view.visibleRanges) {
    const start = Math.max(from, body);
    const text = view.state.doc.sliceString(start, to);
    for (const m of text.matchAll(HL)) {
      const a = start + m.index!, b = a + m[0].length;
      if (source) out.push(hl.range(a, b));
      else out.push(hide.range(a, a + 2), hl.range(a + 2, b - 2), hide.range(b - 2, b));
    }
  }
  return Decoration.set(out, true);
}

/** Draws `==words==` with a marker and hides the `==`. Only the visible ranges are scanned, and only when the text, the viewport or the mode changed (a caret move redraws nothing). */
export const highlightPlugin: Extension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) { this.decorations = highlightDecos(view); }
    update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged || u.transactions.some((t) => t.reconfigured)) this.decorations = highlightDecos(u.view); }
  },
  { decorations: (v) => v.decorations },
);

// ---- The shortcuts ----

/** What the shortcuts call: the host (the bar's store) decides how each is done. */
export interface Shortcuts {
  mark(kind: MarkKind): void;
  heading(level: 1 | 2 | 3): void;
  move(dir: -1 | 1): void;
  /** Esc with a caret: selects the block under it. */
  selectBlock(): void;
  /** Whether whole blocks are selected (then Esc leaves the editor). */
  blocksSelected(): boolean;
  leave(): void;
  insert(): void;
  link(): void;
  /** These two act on the block under the caret; false (no block) leaves the key to the browser. */
  duplicate(): boolean;
  remove(): boolean;
  copyLink(): void;
}

/** Highlight is a Shift-Mod chord on a physical key (Y, or H): with Ctrl some browsers report a plain "y", which a keymap would read as Ctrl+Y (redo), and Safari keeps Shift-Cmd-H for Home. */
const isHighlight = (e: KeyboardEvent, mac: boolean): boolean => (mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey) && e.shiftKey && !e.altKey && (e.code === "KeyY" || e.code === "KeyH");

/**
 * The standard shortcuts while writing (the existing app's key map, issue #39 there), at a higher precedence than the text layer's own so a block version of a key wins:
 * Mod-B bold, Mod-I italic, Shift-Mod-X strikethrough, Mod-E code, Shift-Mod-Y highlight, Mod-K link (with words selected; with none it is the palette, the window's), Mod-Alt-1/2/3 headings,
 * Alt-Up and Alt-Down (Mod-Alt-Up and Down on a Mac) move the block, (Tab and Shift-Tab nest a list item: the editor's own `indentItem`, unchanged), Mod-/ insert, Mod-D duplicate, Shift-Mod-K delete the block, Alt-Shift-Mod-L copy the block link,
 * Esc selects the block and, with blocks selected, leaves the editor. A mark key does nothing without selected words (the browser keeps it). `Mod` is Command on a Mac and Control elsewhere.
 */
export function shortcuts(s: Shortcuts, mac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform)): Extension {
  const run = (f: () => void) => () => (f(), true);
  const marked = (kind: MarkKind) => (v: EditorView) => (v.state.selection.main.empty ? false : (s.mark(kind), true));
  return [
    highlightPlugin,
    Prec.highest(EditorView.domEventHandlers({ keydown(ev, v) { if (!isHighlight(ev, mac) || v.state.selection.main.empty) return false; ev.preventDefault(); s.mark("highlight"); return true; } })),
    Prec.high(keymap.of([
      { key: "Mod-b", run: marked("bold") },
      { key: "Mod-i", run: marked("italic") },
      { key: "Mod-Shift-x", run: marked("strike") },
      { key: "Mod-e", run: marked("code") },
      { key: "Mod-k", run: (v) => (v.state.selection.main.empty ? false : (s.link(), true)) },
      { key: "Mod-Alt-1", run: run(() => s.heading(1)) },
      { key: "Mod-Alt-2", run: run(() => s.heading(2)) },
      { key: "Mod-Alt-3", run: run(() => s.heading(3)) },
      { mac: "Mod-Alt-ArrowUp", key: "Alt-ArrowUp", run: run(() => s.move(-1)) },
      { mac: "Mod-Alt-ArrowDown", key: "Alt-ArrowDown", run: run(() => s.move(1)) },
      { key: "Mod-/", run: run(s.insert) },
      { key: "Mod-d", run: () => s.duplicate() },
      { key: "Mod-Shift-k", run: () => s.remove() },
      { key: "Mod-Alt-Shift-l", run: run(s.copyLink) },
      { key: "Escape", run: (v) => (s.blocksSelected() ? (s.leave(), true) : v.state.selection.main.empty ? (s.selectBlock(), true) : false) },
    ])),
  ];
}
