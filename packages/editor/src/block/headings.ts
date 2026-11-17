/** The headings of a note, for the contents rail: level, text and where each starts. Cost: one walk of the parsed tree. */
import { syntaxTree } from "@codemirror/language";
import type { EditorState } from "@codemirror/state";

export interface Heading { level: number; text: string; from: number }

export function headingsOf(state: EditorState): Heading[] {
  const out: Heading[] = [];
  syntaxTree(state).iterate({
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
