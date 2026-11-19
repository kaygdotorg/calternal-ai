/**
 * Frontmatter (the `---` block of properties at the top of a file) is never
 * shown in the default mode: people never need to edit it there (ADR 0025).
 * It stays in the file, untouched, and is visible in source mode.
 */

import { type EditorState, type Extension, RangeSet, StateField, type Text } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView } from '@codemirror/view'

// Only a block that ends with a line break counts: otherwise there would be
// no visible line to type into, and typing would land inside the hidden
// properties. The server always writes that line break. The block must end
// within the first 64 KB.
const LIMIT = 64 * 1024
const OPEN = /^---\r?$/
const CLOSE = /^---[ \t]*\r?$/

/** Each document's answer: many extensions ask on every keystroke. */
const ends = new WeakMap<Text, number>()

/** Where the note body starts: the end of the frontmatter, or 0. */
export function frontmatterEnd(state: EditorState): number {
  const doc = state.doc
  let end = ends.get(doc)
  if (end !== undefined) return end
  end = 0
  // Line by line, never copying the text: the closing `---` is on line 3 or later (line 2
  // can't close an empty block: `---` right after the opening one is text) and ends with a
  // line break.
  if (doc.lines > 2 && OPEN.test(doc.line(1).text)) {
    for (let i = 3; i < doc.lines; i++) {
      const line = doc.line(i)
      if (line.to + 1 > LIMIT) break
      if (CLOSE.test(line.text)) {
        end = line.to + 1
        break
      }
    }
  }
  ends.set(doc, end)
  return end
}

function build(state: EditorState): DecorationSet {
  const end = frontmatterEnd(state)
  if (end === 0) return Decoration.none
  // Hide the block's whole lines; the body starts on the next line.
  const last = state.doc.lineAt(end - 1)
  return Decoration.set([Decoration.replace({ block: true }).range(0, last.to)])
}

const field = StateField.define<DecorationSet>({
  create: build,
  update: (deco, tr) => (tr.docChanged ? build(tr.state) : deco),
  provide: (f) => [
    EditorView.decorations.from(f),
    EditorView.atomicRanges.of((view) => view.state.field(f, false) ?? RangeSet.empty),
  ],
})

export const hiddenFrontmatter: Extension = field

/** Whether the properties are hidden (the default mode) rather than shown (source mode). */
export const frontmatterHidden = (state: EditorState): boolean => state.field(field, false) !== undefined
