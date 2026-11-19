/**
 * The two editing modes (ADR 0019):
 *
 * - `default`: syntax hidden, the note looks like a document, nested lists
 *   drawn with bullet-journal tree lines.
 * - `source`: the raw markdown, every character visible, frontmatter too.
 *
 * Switching never changes the text, only how it's drawn.
 */

import { Compartment, type Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { treeLines } from '../lists/tree-lines'
import { breaks } from './breaks'
import { hiddenSyntax } from './hidden'
import { hiddenBlockIds } from './block-ids'

export type EditorMode = 'default' | 'source'

const compartment = new Compartment()

function forMode(mode: EditorMode): Extension {
  // Source mode shows the file as it is, spaces included, so no tree lines there, and tables show their pipes.
  return mode === 'default' ? [hiddenBlockIds, hiddenSyntax, treeLines, breaks] : [hiddenBlockIds]
}

export function modeExtension(mode: EditorMode): Extension {
  return compartment.of(forMode(mode))
}

/** Changes the drawing mode and drops an already-loaded table extension from source mode. */
export function setMode(view: EditorView, mode: EditorMode): void {
  // The app may draw source mode in another font, so lines can rewrap. The caret's line stays
  // where it was on screen (or, with the caret out of sight, the top line does): nothing seems
  // to jump, only the letters change.
  const head = view.state.selection.main.head
  const box = view.scrollDOM.getBoundingClientRect()
  const at = view.coordsAtPos(head)
  const caretShown = at != null && at.top >= box.top && at.bottom <= box.bottom
  const snapshot = view.scrollSnapshot()
  // Set first: extensions outside the engine (the app's highlight) read it while redrawing.
  view.dom.dataset.mode = mode
  view.dispatch({ effects: compartment.reconfigure(forMode(mode)) })
  // The table compartment is appended only after a table is needed; do not load its chunk for plain notes.
  if (mode === 'source' && view.dom.dataset.tablesActive === 'true') {
    void import('../tables/lazy')
      .then((tables) => {
        if (view.dom.isConnected && view.dom.dataset.mode === 'source') tables.deactivate(view)
      })
      .catch(() => {})
  }
  // The caret may sit in the properties (source mode shows them): coming back to the default mode, the guard moves it to the body.
  view.dispatch({ selection: view.state.selection })
  if (!caretShown) return view.dispatch({ effects: snapshot })
  holdLine(view, head, at.top)
}

/**
 * Keeps the line at `pos` at `y` on screen while the editor settles new line heights (it measures
 * them over several frames). Only the editor's own scroller moves: scrolling "into view" could
 * move the page around it. It lets go after half a second, or as soon as you scroll or type.
 */
function holdLine(view: EditorView, pos: number, y: number): void {
  let frames = 30
  let done = false
  const stop = () => {
    done = true
    for (const e of ['wheel', 'touchstart', 'keydown', 'pointerdown']) view.scrollDOM.removeEventListener(e, stop)
  }
  for (const e of ['wheel', 'touchstart', 'keydown', 'pointerdown']) view.scrollDOM.addEventListener(e, stop, { passive: true })
  const hold = {
    read: (v: EditorView) => (v.coordsAtPos(pos)?.top ?? y) - y,
    write: (moved: number, v: EditorView) => {
      if (done) return
      if (Math.abs(moved) >= 0.5) {
        v.scrollDOM.scrollTop += moved
        // The drawn caret was placed before this: draw it again where the text is now.
        v.dispatch({ selection: v.state.selection })
      }
      if (--frames > 0) requestAnimationFrame(() => v.requestMeasure(hold))
      else stop()
    },
  }
  // A frame later, so the editor has first kept its own top line in place, as it always does.
  requestAnimationFrame(() => view.requestMeasure(hold))
}
