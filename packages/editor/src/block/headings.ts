/** The headings of a note, for the contents rail: level, text and where each starts. Cost: one walk of the parsed tree. */
import { syntaxTree } from "@codemirror/language";
import { IterMode } from "@lezer/common";
import type { EditorState } from "@codemirror/state";

/** One heading of the document: its level (1 to 6), its text and where it starts, for the contents rail. */
export interface Heading { level: number; text: string; from: number }

/** The headings of the document in order, from the syntax tree already parsed (no re-parse). Cost: one walk of the tree, so the host debounces it. */
export function headingsOf(state: EditorState): Heading[] {
  const out: Heading[] = [];
  syntaxTree(state).iterate({
    mode: IterMode.IgnoreMounts, // never walk into a code fence's own grammar: nothing here looks at it, and a nested tree is many more nodes per key
    enter(n) {
      const m = /^(?:ATX|Setext)Heading([1-6])$/.exec(n.name);
      if (!m) return n.name === "Document" || n.name.endsWith("List") || n.name === "ListItem" || n.name === "Blockquote" ? undefined : false;
      const line = state.doc.sliceString(n.from, n.to);
      out.push({ level: +m[1]!, text: line.replace(/^#{1,6}\s+/, "").replace(/\s+#+\s*$/, "").split("\n")[0]!.trim(), from: n.from });
      return false;
    },
  });
  return out;
}
