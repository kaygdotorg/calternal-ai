/**
 * How selected blocks look (#538): a block selection never looks like selected words.
 *
 * A block selection is, underneath, a text selection across the block (the app keeps it so),
 * which CodeMirror would paint like any other. While blocks are selected that paint and the caret
 * step aside, and each selected block wears a rounded accent tint reaching a little past its text:
 * the look kayg approved for a picked block (#397), without the phone's circles.
 *
 * The look lasts while the selection is still the one it was given; any other selection ends it.
 * It never animates: it comes from a key or a click, and shows at once.
 */

import { type Extension, StateEffect, StateField } from '@codemirror/state'
import { EditorView, type LayerMarker, RectangleMarker, type ViewUpdate, layer } from '@codemirror/view'
import { blockBox, nudged, watchLayout } from './box'

export interface Span {
  from: number
  to: number
}

/** The tint reaches this far past the text on each side. */
const PAST = 8

const show = StateEffect.define<readonly Span[] | null>()

/** The effect that gives these blocks the selected look; send it with the selection across them. */
export const showBlocks = (spans: readonly Span[] | null) => show.of(spans && spans.length ? spans.map((s) => ({ from: s.from, to: s.to })) : null)

const hull = (spans: readonly Span[]): Span => ({ from: spans[0]!.from, to: spans[spans.length - 1]!.to })

const shown = StateField.define<readonly Span[] | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(show)) return e.value
    if (!value) return null
    const mapped = tr.docChanged ? value.map((s) => ({ from: tr.changes.mapPos(s.from, 1), to: tr.changes.mapPos(s.to, -1) })) : value
    const sel = tr.state.selection.main
    const h = hull(mapped)
    // A deleted block is gone, and an empty selection holds no blocks.
    if (sel.empty || mapped.some((s) => s.to <= s.from)) return null
    return sel.from === h.from && sel.to === h.to ? mapped : null
  },
})

/** The blocks showing as selected, or null. */
export const shownBlocks = (state: import('@codemirror/state').EditorState): readonly Span[] | null => state.field(shown, false) ?? null

const changed = (u: ViewUpdate) => nudged(u) || u.docChanged || u.viewportChanged || u.geometryChanged || u.startState.field(shown, false) !== u.state.field(shown, false)

const tint = layer({
  above: false,
  class: 'cm-block-sel-layer',
  markers(view) {
    const spans = view.state.field(shown, false)
    if (!spans) return []
    const scroller = view.scrollDOM.getBoundingClientRect()
    const ox = scroller.left - view.scrollDOM.scrollLeft
    const oy = scroller.top - view.scrollDOM.scrollTop
    const content = view.contentDOM.getBoundingClientRect()
    const out: LayerMarker[] = []
    for (const s of spans) {
      if (s.to < view.viewport.from || s.from > view.viewport.to) continue
      const box = blockBox(view, s)
      if (!box) continue
      out.push(new RectangleMarker('cm-block-sel', content.left - PAST - ox, box.top - oy, content.width + 2 * PAST, box.bottom - box.top))
    }
    return out
  },
  update: changed,
})

const theme = EditorView.baseTheme({
  '&.cm-blocks-shown .cm-selectionBackground': { background: 'transparent !important' },
  '&.cm-blocks-shown .cm-content ::selection': { background: 'transparent !important' },
  '&.cm-blocks-shown .cm-content': { caretColor: 'transparent !important' },
  '&.cm-blocks-shown .cm-cursor, &.cm-blocks-shown .cm-fat-cursor': { display: 'none' },
  '& .cm-block-sel': {
    background: 'color-mix(in oklab, var(--accent, #b45309) 16%, transparent)',
    borderRadius: '10px',
  },
  // A phone's own picked look (#397) draws its tint itself.
  '&.cm-picking .cm-block-sel': { display: 'none' },
})

const shownClass = EditorView.editorAttributes.compute([shown], (state) => ({ class: state.field(shown) ? 'cm-blocks-shown' : '' }))

export const selectedBlocks: Extension = [shown, tint, shownClass, theme, watchLayout((state) => state.field(shown, false) != null)]
