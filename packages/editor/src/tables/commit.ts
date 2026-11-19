/**
 * How a table grid writes into the note: changed cell rows line by line, a structure change as the whole table, and
 * the caret leaving the table for the text above or below. Each function checks that the note still holds the text
 * the grid last saw, and answers `false` when it does not, so the grid can take the note's text in instead of
 * overwriting it. Split out of `grid.ts`.
 */

import { Transaction } from '@codemirror/state'
import type { EditorView } from '@codemirror/view'
import { type Tbl, textFor } from './model'
import { type GridRevision, tableCellEdit } from './revision'

/**
 * Applies the changed cell rows (and any first-edit delimiter tidy) after checking only those document lines.
 * `false`: the note's lines are not the ones the revision expects.
 */
export function writeCellRows(view: EditorView, revision: GridRevision, pending: ReadonlySet<number>, from: number): boolean {
  const state = view.state
  const first = state.doc.lineAt(from)
  if (first.from !== from) return false
  const changes: { from: number; to: number; insert: string }[] = []
  for (const index of [...pending].sort((a, b) => a - b)) {
    const line = state.doc.line(first.number + index)
    const expected = revision.appliedLines[index]
    const insert = revision.lines[index]
    if (expected === undefined || insert === undefined || line.text !== expected || line.from < from || line.to > from + revision.textLength) {
      return false
    }
    if (insert !== expected) {
      changes.push({ from: line.from, to: line.to, insert })
    }
  }
  if (!changes.length) return true
  const version = revision.version
  view.dispatch({
    changes,
    annotations: [Transaction.userEvent.of('input.table.cell'), tableCellEdit.of({ revision, version, from })],
  })
  return true
}

/**
 * Replaces the whole table for a structural edit, where a full serialization is expected. `settle` runs just before
 * the note changes, so the grid's own flags are already clear when it hears about the change. `false`: stale.
 */
export function writeStructure(view: EditorView, revision: GridRevision, t: Tbl, from: number, settle: () => void): boolean {
  const current = revision.appliedLines.join('\n')
  const to = from + revision.textLength
  if (view.state.sliceDoc(from, to) !== current) return false
  const next = textFor(revision.lines.join('\n'), t)
  settle()
  if (next !== current) view.dispatch({ changes: { from, to, insert: next }, userEvent: 'input.table' })
  return true
}

/** Out of the table, whose text is `from`..`to`, to the line above or below (a new line after the last one). */
export function leaveTable(view: EditorView, from: number, to: number, dir: -1 | 1): void {
  const doc = view.state.doc
  let pos: number
  if (dir < 0) {
    if (from === 0) {
      view.dispatch({ changes: { from: 0, insert: '\n\n' }, selection: { anchor: 0 } })
      pos = 0
    } else pos = from - 1
  } else if (to >= doc.length) {
    view.dispatch({ changes: { from: to, insert: '\n\n' } })
    pos = to + 2
  } else {
    // The blank line under a table is what ends it: a line typed straight under the last row would become one more row.
    // So the caret goes to the line after that blank one, which is made if the text below starts right away.
    const gap = doc.lineAt(to + 1)
    const after = gap.number < doc.lines ? doc.line(gap.number + 1) : null
    if (gap.text.trim()) pos = to + 1
    else if (after && !after.text.trim()) pos = after.from
    else {
      view.dispatch({ changes: { from: gap.to, insert: '\n' } })
      pos = gap.to + 1
    }
  }
  ;(document.activeElement as HTMLElement | null)?.blur()
  view.focus()
  view.dispatch({ selection: { anchor: Math.min(pos, view.state.doc.length) }, scrollIntoView: true })
}
