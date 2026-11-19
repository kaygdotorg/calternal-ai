/**
 * Frontmatter (the `---` block of properties at the top of a note) is never drawn in the note (ADR 0025 of the existing app: people do not edit it in the document). It stays in the file,
 * untouched, and a note's other settings (hidden blocks, table widths) live in it. This is the lazy piece: `frontmatterEnd` is pure and also used by anything that must skip the block;
 * `frontmatter` hides the block as one replaced range. The block must end with a line break and within the first 64 KB, so there is always a visible line to type into.
 */
import { type EditorState, type Extension, RangeSet, StateField } from "@codemirror/state";
import { frontmatterReport } from "./report";
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from "@codemirror/view";

import { frontmatterEnd } from "./frontmatter-end";
export { frontmatterEnd };

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


/** The note's frontmatter as `key: value` lines (a note's settings: `calternal-*` keys). Lines that are not `key: value` are skipped; a value keeps its text after the first colon, trimmed, without quotes around it. Cost: the block only, never the body. */
export function frontmatterMap(state: EditorState): Record<string, string> {
  const end = frontmatterEnd(state);
  const out: Record<string, string> = {};
  if (!end) return out;
  const doc = state.doc;
  const last = doc.lineAt(end - 1).number - 1;
  for (let i = 2; i <= last; i++) {
    const m = /^([A-Za-z][\w-]*):[ \t]*(.*?)[ \t]*\r?$/.exec(doc.line(i).text);
    if (m) out[m[1]!] = m[2]!.replace(/^(["'])(.*)\1$/, "$2");
  }
  return out;
}

/**
 * Reports the frontmatter map to the host (the `frontmatterReport` facet) once when the piece is built, and then only when a change touches the block. Cost per keystroke in the body: one comparison
 * of the change's start against the block's cached end; nothing is parsed and nothing is drawn. Part of the lazy frontmatter piece: a note with no frontmatter never loads it.
 */
const watch = ViewPlugin.fromClass(class {
  constructor(view: EditorView) { view.state.facet(frontmatterReport)?.(frontmatterMap(view.state)); }
  update(u: ViewUpdate) {
    if (!u.docChanged) return;
    const report = u.state.facet(frontmatterReport);
    if (!report) return;
    const reach = Math.max(frontmatterEnd(u.startState), frontmatterEnd(u.state));
    // A change before the block's end touches it; one that adds a block at the top (reach 0 before, more after) is covered by the max.
    if (reach && u.changes.touchesRange(0, reach)) report(frontmatterMap(u.state));
  }
});

/** The extension that hides the frontmatter block and reports its settings to the host (the `frontmatterReport` facet). */
export const frontmatter: Extension = [field, watch];
