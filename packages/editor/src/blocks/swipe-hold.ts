/**
 * Holding a block selection against the phone's own text selection (#397).
 *
 * On an iPhone a sideways drag over text in an editor that has the keyboard up is also iOS's way
 * to select words, and a page can't veto that once it has started (only `touchstart` can, and that
 * would cost scrolling and tapping). So while a swipe is selecting a block, and for a moment after,
 * any selection change that comes from the screen (`select`, `select.pointer`) is turned back into
 * the selection we had. Each one it turns back keeps the hold a little longer, so a selection that
 * keeps following the finger keeps being undone, up to a few seconds.
 *
 * While the finger swipes, the text also can't be selected at all (`data-swiping` on the
 * scroller): the phone's selection has nothing to grow into.
 */

import { EditorState, type Extension, StateEffect, StateField } from '@codemirror/state'
import { EditorView } from '@codemirror/view'

/** How long the hold lasts after the last selection it turned back. */
export const QUIET = 400
/** The longest a hold can last, from when it starts. */
const LONGEST = 3000

interface Hold {
  until: number
  last: number
}

const NONE: Hold = { until: 0, last: 0 }

/** Starts (or ends, with `until: 0`) a hold. */
const setHold = StateEffect.define<Hold>()

const hold = StateField.define<Hold>({
  create: () => NONE,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setHold)) value = e.value
    return value
  },
})

const keepSelection = EditorState.transactionFilter.of((tr) => {
  const h = tr.startState.field(hold, false)
  if (!h || !tr.selection || tr.docChanged || !tr.isUserEvent('select')) return tr
  const now = Date.now()
  if (now > h.until) return tr
  // The same selection again: CodeMirror puts it back on the page, over the phone's.
  const until = Math.min(h.last, Math.max(h.until, now + QUIET))
  return { selection: tr.startState.selection, effects: setHold.of({ until, last: h.last }) }
})

const noSelectWhileSwiping = EditorView.baseTheme({
  '.cm-scroller[data-swiping] .cm-content': {
    userSelect: 'none',
    WebkitUserSelect: 'none',
    WebkitTouchCallout: 'none',
  },
})

/** The editor side of the hold; `blockChrome` adds it. */
export const swipeHold: Extension = [hold, keepSelection, noSelectWhileSwiping]

/** Holds the selection for `ms` from now (at most a few seconds in all). */
export function holdFor(view: EditorView, ms: number): void {
  const now = Date.now()
  const current = view.state.field(hold, false)
  const last = current && current.until > now ? current.last : now + LONGEST
  view.dispatch({ effects: setHold.of({ until: Math.min(now + ms, last), last }) })
}

/** Lets go of the selection right away. */
export function release(view: EditorView): void {
  const current = view.state.field(hold, false)
  if (current && current.until > 0) view.dispatch({ effects: setHold.of(NONE) })
}
