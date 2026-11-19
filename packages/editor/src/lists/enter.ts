/**
 * Return in a list: one press, one new line (#225).
 *
 * CodeMirror's markdown Return keeps a "loose" list loose: once two items
 * have a blank line between them, every Return adds a blank line before
 * the new bullet, and Return on an empty item pushes it down a line
 * instead of ending the list. Notes here are written on the go, a blank
 * line slips in easily, and from then on each Return made two line breaks.
 *
 * So Return here always:
 * - puts the new bullet on the very next line, never after a blank one;
 * - ends the list on an empty item (or steps out one level when nested).
 *
 * Everything else (numbering, quotes, tasks, nesting) is CodeMirror's own.
 */

import { insertNewlineContinueMarkupCommand } from '@codemirror/lang-markdown'
import { type ChangeSpec, EditorSelection, type StateCommand, type Transaction } from '@codemirror/state'

// An empty item always ends the list, even after a blank line.
const continueMarkup = insertNewlineContinueMarkupCommand({ nonTightLists: false })

// What CodeMirror inserts to keep a loose list loose: a line break, a blank
// line (only spaces or quote marks), then the line break before the bullet.
const LOOSE = /^(\r?\n)[ \t>]*(?=\r?\n)/

export const continueList: StateCommand = ({ state, dispatch }) => {
  const got: { tr?: Transaction } = {}
  if (!continueMarkup({ state, dispatch: (t) => (got.tr = t) }) || !got.tr) return false
  const tr = got.tr
  const changes: ChangeSpec[] = []
  // Where each kept change ends in the new text, and how much shorter it got.
  const cuts: { at: number; by: number }[] = []
  tr.changes.iterChanges((from, to, _fromB, toB, text) => {
    const insert = text.toString()
    const m = LOOSE.exec(insert)
    const cut = m ? m[0].length : 0
    changes.push({ from, to, insert: insert.slice(cut) })
    if (cut) cuts.push({ at: toB, by: cut })
  })
  if (!cuts.length) {
    dispatch(tr)
    return true
  }
  // Move each caret back by the blank lines dropped at or before it.
  const shift = (pos: number) => pos - cuts.reduce((n, c) => (c.at <= pos ? n + c.by : n), 0)
  const selection = EditorSelection.create(
    tr.newSelection.ranges.map((r) => EditorSelection.cursor(shift(r.head))),
    tr.newSelection.mainIndex,
  )
  dispatch(state.update({ changes, selection, scrollIntoView: true, userEvent: 'input' }))
  return true
}
