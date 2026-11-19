/** Block IDs stay in the document but never appear in either editing mode. */
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import type { Range } from '@codemirror/state'
import { visibleText, type HiddenTextRange } from './visible-text'

const hiddenMarker = Decoration.replace({ attributes: { 'aria-hidden': 'true' } })

class HiddenBlockIds {
  decorations: DecorationSet

  constructor(readonly view: EditorView) {
    this.decorations = build(view)
  }

  update(update: ViewUpdate): void {
    if (update.docChanged || update.viewportChanged) this.decorations = build(update.view)
  }
}

function build(view: EditorView): DecorationSet {
  const ranges: Range<Decoration>[] = []
  for (const visible of view.visibleRanges) {
    for (let pos = visible.from; pos <= visible.to; ) {
      const line = view.state.doc.lineAt(pos)
      const hidden: HiddenTextRange[] = []
      visibleText(line.text, hidden)
      for (const span of hidden) {
        ranges.push(hiddenMarker.range(line.from + span.from, line.from + span.to))
      }
      pos = line.to + 1
    }
  }
  return Decoration.set(ranges, true)
}

const plugin = ViewPlugin.fromClass(HiddenBlockIds, {
  decorations: (value) => value.decorations,
  provide: (value) => EditorView.atomicRanges.of((view) => view.plugin(value)?.decorations ?? Decoration.none),
})

export { plugin as hiddenBlockIds }
