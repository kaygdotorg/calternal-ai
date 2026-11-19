/**
 * The keyboard of a table grid. Two cases: the caret is in a cell (Tab, Enter, arrows at the cell's edges, Backspace
 * in an empty last row, Escape), or a whole row or column is picked (arrows move or reorder it, Backspace removes it).
 * The keys only decide what to do and ask the grid, through `KeyHost`, to do it. Split out of `grid.ts`.
 *
 *   Tab / Shift-Tab    next / previous cell; Tab in the last cell adds a row
 *   Enter              the cell below; in the last row it adds a row
 *   arrows             across cells at a cell's edges; up and down leave the table at its top and bottom
 *   Backspace          in an empty last row, takes the row away
 */

import type { EditorView } from '@codemirror/view'
import { atEdge, onEdgeLine, rOf, type Where } from './cells'
import type { Tbl } from './model'
import type { Cursor } from './paint'
import type { TableOp } from './state'

/** What the keys need from the grid. */
export interface KeyHost {
  readonly view: EditorView
  /** The table as it is now. */
  table(): Tbl
  cursor(): Cursor | null
  go(r: number, c: number, where?: Where): void
  leaveTo(dir: -1 | 1): void
  addRow(at: number): void
  enterKey(shift: boolean, r: number, c: number): void
  flush(): void
  /** Takes row `r` away, then puts the caret at `go`. */
  dropRow(r: number, go: () => void): void
  op(op: TableOp): void
  pick(kind: 'row' | 'col', r: number, c: number): void
}

/** A key pressed with the caret in cell `t`. */
export function onCellKey(e: KeyboardEvent, t: HTMLElement, host: KeyHost): void {
  const r = rOf(t)
  const c = +t.dataset.c!
  const tbl = host.table()
  const plainKey = !e.metaKey && !e.ctrlKey && !e.altKey
  const lastR = tbl.rows.length - 1
  const lastC = tbl.align.length - 1
  const k = e.key
  if ((e.metaKey || e.ctrlKey) && !e.altKey && /^[zy]$/i.test(k)) {
    // Undo and redo are the note's (one history, ADR 0022), not the cell's own.
    e.preventDefault()
    host.flush()
    host.view.contentDOM.dispatchEvent(new KeyboardEvent('keydown', { key: k, code: e.code, ctrlKey: e.ctrlKey, metaKey: e.metaKey, shiftKey: e.shiftKey, bubbles: true, cancelable: true }))
    return
  }
  if (k === 'Tab' && !e.metaKey && !e.ctrlKey) {
    e.preventDefault()
    if (e.shiftKey) {
      if (c > 0) host.go(r, c - 1)
      else if (r > 0) host.go(r - 1, lastC)
      else host.leaveTo(-1)
    } else if (c < lastC) host.go(r, c + 1)
    else if (r < lastR) host.go(r + 1, 0)
    else {
      host.addRow(tbl.rows.length)
      host.go(tbl.rows.length, 0)
    }
  } else if (k === 'Enter' && plainKey) {
    e.preventDefault()
    host.enterKey(e.shiftKey, r, c)
  } else if (k === 'ArrowLeft' && plainKey && !e.shiftKey && atEdge(t, false)) {
    e.preventDefault()
    if (c > 0) host.go(r, c - 1, 'end')
    else if (r > 0) host.go(r - 1, lastC, 'end')
    else host.leaveTo(-1)
  } else if (k === 'ArrowRight' && plainKey && !e.shiftKey && atEdge(t, true)) {
    e.preventDefault()
    if (c < lastC) host.go(r, c + 1, 'start')
    else if (r < lastR) host.go(r + 1, 0, 'start')
    else host.leaveTo(1)
  } else if (k === 'ArrowUp' && plainKey && !e.shiftKey && onEdgeLine(t, -1)) {
    e.preventDefault()
    if (r > 0) host.go(r - 1, c, 'end')
    else host.leaveTo(-1)
  } else if (k === 'ArrowDown' && plainKey && !e.shiftKey && onEdgeLine(t, 1)) {
    e.preventDefault()
    if (r < lastR) host.go(r + 1, c, 'end')
    else host.leaveTo(1)
  } else if (k === 'Backspace' && plainKey && r === lastR && r > 1 && tbl.rows[r]!.every((x) => !x.trim()) && !t.textContent) {
    // An empty last row: Backspace takes it away and the caret goes back to the row above.
    e.preventDefault()
    host.dropRow(r, () => host.go(r - 1, c, 'end'))
  } else if (k === 'Escape') {
    t.blur()
    host.view.focus()
  }
}

/** A key pressed while a whole row or column is picked: keys act on it. */
export function onPickedKey(e: KeyboardEvent, host: KeyHost): void {
  const cur = host.cursor()
  if (!cur || cur.kind === 'cell') return
  const row = cur.kind === 'row'
  const k = e.key
  if (k === 'Backspace' || k === 'Delete') {
    e.preventDefault()
    host.op(row ? 'row-del' : 'col-del')
  } else if (k === 'Enter') {
    e.preventDefault()
    host.go(row ? cur.r : 0, row ? 0 : cur.c, 'all')
  } else if (k === 'Escape') {
    e.preventDefault()
    host.op('done')
  } else if (k === (row ? 'ArrowUp' : 'ArrowLeft') || k === (row ? 'ArrowDown' : 'ArrowRight')) {
    e.preventDefault()
    const d = k === 'ArrowUp' || k === 'ArrowLeft' ? -1 : 1
    if (e.shiftKey || e.altKey || e.metaKey) host.op(row ? (d < 0 ? 'move-up' : 'move-down') : d < 0 ? 'move-left' : 'move-right')
    else if (row) host.pick('row', Math.max(1, Math.min(host.table().rows.length - 1, cur.r + d)), 0)
    else host.pick('col', 0, Math.max(0, Math.min(host.table().align.length - 1, cur.c + d)))
  }
}
