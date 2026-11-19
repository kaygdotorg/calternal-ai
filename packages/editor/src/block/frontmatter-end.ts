/** Where a note's frontmatter ends. Pure and tiny, so the editor and the block commands can skip the block without loading the lazy piece that hides it (frontmatter.ts). */
import type { EditorState, Text } from "@codemirror/state";

const LIMIT = 64 * 1024;
const ends = new WeakMap<Text, number>(); // each document's answer: asked on every update

/** Where the note body starts: the end of the frontmatter (just after its closing line break), or 0 when there is none. Line by line, never copying the text. */
export function frontmatterEnd(state: EditorState): number {
  const doc = state.doc;
  let end = ends.get(doc);
  if (end !== undefined) return end;
  end = 0;
  if (doc.lines > 2 && /^---\r?$/.test(doc.line(1).text)) {
    for (let i = 3; i < doc.lines; i++) {
      const l = doc.line(i);
      if (l.to + 1 > LIMIT) break;
      if (/^---[ \t]*\r?$/.test(l.text)) { end = l.to + 1; break; }
    }
  }
  ends.set(doc, end);
  return end;
}

