/**
 * Frontmatter (the `---` block of properties at the top of a note) is never drawn in the note (ADR 0025 of the existing app: people do not edit it in the document). It stays in the file,
 * untouched, and a note's other settings (hidden blocks, table widths) live in it. This is the lazy piece: `frontmatterEnd` is pure and also used by anything that must skip the block;
 * `frontmatter` hides the block as one replaced range. The block must end with a line break and within the first 64 KB, so there is always a visible line to type into.
 */
import { type EditorState, type Extension, RangeSet, StateField, type Text } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView } from "@codemirror/view";

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

function build(state: EditorState): DecorationSet {
  const end = frontmatterEnd(state);
  if (!end) return Decoration.none;
  return Decoration.set([Decoration.replace({ block: true }).range(0, state.doc.lineAt(end - 1).to)]);
}

const field = StateField.define<DecorationSet>({
  create: build,
  update: (d, tr) => (tr.docChanged ? build(tr.state) : d),
  provide: (f) => [EditorView.decorations.from(f), EditorView.atomicRanges.of((v) => v.state.field(f, false) ?? RangeSet.empty)],
});

/** The extension that hides the frontmatter block. */
export const frontmatter: Extension = field;
