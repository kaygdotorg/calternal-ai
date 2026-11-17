/**
 * List behaviour on the person's own keystrokes only: Enter continues a list item (a bullet, the next number, a fresh unchecked task) and ends the list on an empty item;
 * Tab and Shift-Tab indent and outdent an item by two spaces. Pure commands over `{state, dispatch}`, so they are tested without a view.
 */
import type { EditorState, Transaction } from "@codemirror/state";
import { endMarker } from "./ids";

type Target = { state: EditorState; dispatch: (tr: Transaction) => void };
const ITEM = /^(\s*)([-*+]|\d+[.)])( +)(\[[ xX]\] +)?/;

/** Enter in a list item. Returns false when the caret is not in one (the default Enter runs). */
export function continueList({ state, dispatch }: Target): boolean {
  const sel = state.selection.main;
  if (!sel.empty) return false;
  const line = state.doc.lineAt(sel.head);
  const m = ITEM.exec(line.text);
  if (!m || sel.head < line.from + m[0].length) return false;
  const mk = endMarker(line.text); // a block ID marker ends the line: it is not content, and it stays with the first part when the item splits (ADR 0056 B3)
  const rest = line.text.slice(m[0].length, mk ? mk.from : undefined);
  if (!rest.trim()) {
    // An empty item ends the list: its marker goes, leaving an empty line.
    dispatch(state.update({ changes: { from: line.from, to: line.to, insert: "" }, selection: { anchor: line.from }, userEvent: "input" }));
    return true;
  }
  const marker = /^\d/.test(m[2]!) ? `${parseInt(m[2]!, 10) + 1}${m[2]!.slice(-1)}` : m[2]!;
  const insert = `\n${m[1]}${marker}${m[3]}${m[4] ? "[ ] " : ""}`;
  if (mk) {
    const end = line.from + mk.from, at = Math.min(sel.head, end);
    const tail = state.doc.sliceString(at, end);
    dispatch(state.update({ changes: [{ from: at, to: end }, { from: line.to, insert: insert + tail }], selection: { anchor: line.to - (end - at) + insert.length }, userEvent: "input" }));
    return true;
  }
  dispatch(state.update({ changes: { from: sel.head, insert }, selection: { anchor: sel.head + insert.length }, userEvent: "input" }));
  return true;
}

/** Tab (indent) or Shift-Tab (outdent) on the list item the caret is in. */
export function indentItem(dir: 1 | -1) {
  return ({ state, dispatch }: Target): boolean => {
    const line = state.doc.lineAt(state.selection.main.head);
    if (!ITEM.test(line.text)) return false;
    if (dir === 1) dispatch(state.update({ changes: { from: line.from, insert: "  " }, userEvent: "input" }));
    else {
      const n = /^ {1,2}/.exec(line.text)?.[0].length ?? 0;
      if (!n) return true;
      dispatch(state.update({ changes: { from: line.from, to: line.from + n }, userEvent: "input" }));
    }
    return true;
  };
}
