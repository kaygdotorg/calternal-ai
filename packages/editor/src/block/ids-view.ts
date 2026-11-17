/**
 * The view side of block IDs (ids.ts owns what a marker is): hides every marker (atomic, so the caret skips it and Backspace never eats half of one), keeps typing next to a marker from
 * landing after it, gives a new task its ID (B1: tasks get one), and handles pasted text (B3/B4 reading of the existing rules: a pasted copy of an ID already in the note is a copy, so it
 * gets a fresh ID if it is a task and no ID otherwise). Copy and cut put the text without markers on the clipboard, so a copy is unlinked by construction.
 * Lazy: loaded by `features.ts` only when a note holds a marker or a task, so a note without either never pays for it.
 */
import { type Extension, type Range, EditorState, Transaction } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { endMarker, idsIn, isTaskLine, markerFor, markersIn, newId, visibleText } from "./ids";
import { origin } from "./origin";

const hidden = Decoration.replace({ attributes: { "aria-hidden": "true" } });

function build(view: EditorView): DecorationSet {
  const out: Range<Decoration>[] = [];
  const doc = view.state.doc;
  for (const r of view.visibleRanges) for (let p = r.from; p <= r.to; ) {
    const l = doc.lineAt(p);
    if (l.text.includes("^")) for (const m of markersIn(l.text)) out.push(hidden.range(l.from + m.from, l.from + m.to));
    p = l.to + 1;
  }
  return Decoration.set(out, true);
}

const hide = ViewPlugin.fromClass(class {
  decorations: DecorationSet;
  constructor(view: EditorView) { this.decorations = build(view); }
  update(u: ViewUpdate) { if (u.docChanged || u.viewportChanged) this.decorations = build(u.view); }
}, { decorations: (v) => v.decorations, provide: (p) => EditorView.atomicRanges.of((v) => v.plugin(p)?.decorations ?? Decoration.none) });

/** Typing at the very end of a line that ends in a marker lands before the marker, so the marker stays last. A newline stays where it is (Enter after the marker keeps the ID on the first part). */
const keepLast = EditorState.transactionFilter.of((tr) => {
  if (!tr.docChanged || !tr.isUserEvent("input.type")) return tr;
  let one: { from: number; to: number; text: string } | null = null, count = 0;
  tr.changes.iterChanges((fromA, toA, _fromB, _toB, ins) => { count++; one = { from: fromA, to: toA, text: ins.toString() }; });
  if (count !== 1 || !one) return tr;
  const c = one as { from: number; to: number; text: string };
  if (c.from !== c.to || !c.text || c.text.includes("\n")) return tr;
  const line = tr.startState.doc.lineAt(c.from);
  const m = endMarker(line.text);
  if (!m || c.from !== line.from + m.to) return tr;
  const at = line.from + m.from;
  return { changes: { from: at, insert: c.text }, selection: { anchor: at + c.text.length }, userEvent: "input.type", scrollIntoView: true };
});

/** Copy and cut: the clipboard gets the text a person sees. */
const clean = EditorView.clipboardOutputFilter.of((text) => visibleText(text));

/**
 * After the person's change: a task line without a marker gets one, and a pasted block whose ID is already elsewhere in the note loses it (or, for a task, gets a new one). Runs in the next
 * microtask as its own change (the person's typing is never delayed or reordered), joined to the same undo step by the history's grouping delay; never while an input method composes.
 */
const lifecycle = ViewPlugin.fromClass(class {
  queued = false;
  constructor(readonly view: EditorView) {}
  update(u: ViewUpdate) {
    if (!u.docChanged || this.queued) return;
    const own = u.transactions.some((t) => t.docChanged && (t.annotation(origin) ?? "user") !== "remote" && (t.annotation(origin) ?? "user") !== "replay");
    if (!own) return;
    const lines = new Set<number>();
    const pasted = u.transactions.some((t) => t.isUserEvent("input.paste") || t.isUserEvent("input.drop"));
    u.changes.iterChangedRanges((_a, _b, fromB, toB) => { const d = u.state.doc; for (let p = fromB; p <= toB; ) { const l = d.lineAt(p); lines.add(l.number); p = l.to + 1; } });
    if (!lines.size) return;
    this.queued = true;
    const nums = [...lines];
    queueMicrotask(() => { this.queued = false; this.fix(nums, pasted); });
  }
  fix(nums: number[], pasted: boolean) {
    const v = this.view;
    if (v.composing) return;
    const doc = v.state.doc;
    const seen = new Map<string, number>(); // id -> how many times it appears in the note
    let counted = false;
    const count = () => { if (counted) return; counted = true; for (const id of idsIn(v.state)) seen.set(id, (seen.get(id) ?? 0) + 1); };
    const taken = new Set<string>();
    const changes: { from: number; to?: number; insert: string }[] = [];
    for (const n of nums) {
      if (n > doc.lines) continue;
      const l = doc.line(n);
      const task = isTaskLine(l.text);
      const m = endMarker(l.text);
      if (pasted && m) { count(); if ((seen.get(m.id) ?? 0) > 1) { seen.set(m.id, seen.get(m.id)! - 1); changes.push({ from: l.from + m.from, to: l.from + m.to, insert: task ? markerFor(newId(new Set([...seen.keys(), ...taken]))) : "" }); if (task) taken.add(changes.at(-1)!.insert.slice(2)); continue; } }
      if (task && !m) { count(); const id = newId(new Set([...seen.keys(), ...taken])); taken.add(id); changes.push({ from: l.to, insert: markerFor(id) }); }
    }
    if (changes.length) v.dispatch({ changes, annotations: [origin.of("command"), Transaction.addToHistory.of(true)], userEvent: "input.ids" });
  }
});

/** The block ID extension: hide, keep markers last, new tasks, pasted copies, clean clipboard. */
export const ids: Extension = [hide, keepLast, clean, lifecycle];
