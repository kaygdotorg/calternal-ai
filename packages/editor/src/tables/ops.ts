/**
 * The table bar's buttons and the keys that reuse them: add, delete, align, sort and move a row or column. Each
 * operation changes the table model and asks the grid, through `OpHost`, to write and draw it, keeping the caret (or
 * the picked row or column) with what it was on. Split out of `grid.ts`.
 */

import {
  type Align,
  type Tbl,
  deleteCol,
  deleteRow,
  insertCol,
  insertRow,
  moveCol,
  moveRow,
  setAlign,
  sortBy,
  sortIndex,
} from './model'
import { afterInsert, afterMove, afterRemove, moves, playOut } from './motion'
import type { Cursor } from './paint'
import type { TableOp } from './state'
import type { Widths } from './widths'

/** What the operations need from the grid. */
export interface OpHost {
  cursor(): Cursor | null
  table(): Tbl
  /** The rows of the drawn table, for the cells that fade out when a row or column is removed. */
  tableRows(): HTMLTableRowElement[]
  /** The widths a person dragged, by column; changed in place, then saved with `saveWidths`. */
  userCh(): Widths
  /** The sort direction last used per column, so the bar can offer the other one next. */
  last: Map<number, 1 | -1>
  flush(): void
  go(r: number, c: number, where: 'end'): void
  pick(kind: 'row' | 'col', r: number, c: number): void
  saveWidths(): void
  /** Draws and writes the new table; `mv` says where each new row or column came from so the change can be played. */
  edit(t: Tbl, after?: () => void, mv?: { rows?: (i: number) => number; cols?: (c: number) => number }): void
  /** Lets go of the picked cell, row or column and returns the caret to the note. */
  done(): void
}

/** A row or column leaving fades first (for a pointer), then the rest closes up. */
function leaving(cells: HTMLElement[], then: () => void): void {
  if (moves()) void playOut(cells).then(then)
  else then()
}

/** Moves a column, keeping its dragged width with it. */
function reorderCols(host: OpHost, from: number, to: number, after: () => void): void {
  host.userCh().splice(to, 0, host.userCh().splice(from, 1)[0] ?? null)
  host.saveWidths()
  host.edit(moveCol(host.table(), from, to), after, { cols: afterMove(from, to) })
}

/** Runs one bar operation on the picked cell, row or column. */
export function runOp(op: TableOp, host: OpHost): void {
  const cur = host.cursor()
  if (!cur) return
  host.flush()
  const { r, c } = cur
  const cell = cur.kind === 'cell'
  const t = host.table()
  const edit = host.edit.bind(host)
  /** The caret (or the picked row or column) stays with what it was on. */
  const stay = (nr: number, nc: number) => () => (cell ? host.go(nr, nc, 'end') : host.pick(cur.kind as 'row' | 'col', nr, nc))
  switch (op) {
    case 'row-above': {
      const at = Math.max(1, r)
      return edit(insertRow(t, at), stay(r === 0 ? 0 : r + 1, c), { rows: afterInsert(at) })
    }
    case 'row-below':
      return edit(insertRow(t, r + 1), stay(r, c), { rows: afterInsert(r + 1) })
    // The widths are written first: the table's own text follows, and the grid is told the new widths with it.
    case 'col-left':
      host.userCh().splice(c, 0, null)
      host.saveWidths()
      return edit(insertCol(t, c), stay(r, c + 1), { cols: afterInsert(c) })
    case 'col-right':
      host.userCh().splice(c + 1, 0, null)
      host.saveWidths()
      return edit(insertCol(t, c + 1), stay(r, c), { cols: afterInsert(c + 1) })
    case 'row-del':
      if (t.rows.length <= 2 || r < 1) return
      return leaving(Array.from(host.tableRows()[r]?.cells ?? []), () =>
        edit(deleteRow(t, r), stay(Math.min(r, t.rows.length - 2), c), { rows: afterRemove(r) }),
      )
    case 'col-del':
      if (t.align.length <= 1) return
      return leaving(
        host.tableRows().map((tr) => tr.cells[c]).filter((x): x is HTMLTableCellElement => !!x),
        () => {
          host.userCh().splice(c, 1)
          host.saveWidths()
          edit(deleteCol(t, c), stay(r, Math.min(c, t.align.length - 2)), { cols: afterRemove(c) })
        },
      )
    case 'align-l':
    case 'align-c':
    case 'align-r': {
      const a = op.slice(-1) as Align
      // Pressing the one that is on turns it off: back to what the numbers suggest.
      return edit(setAlign(t, c, t.align[c] === a ? '' : a), stay(r, c))
    }
    case 'sort-asc':
    case 'sort-desc': {
      const dir = op === 'sort-asc' ? 1 : -1
      host.last.set(c, dir)
      const order = sortIndex(t, c, dir)
      return edit(sortBy(t, c, dir), stay(r, c), { rows: (i) => (i === 0 ? 0 : order[i - 1]!) })
    }
    case 'move-up':
      if (r > 1) edit(moveRow(t, r, r - 1), stay(r - 1, c), { rows: afterMove(r, r - 1) })
      return
    case 'move-down':
      if (r < t.rows.length - 1) edit(moveRow(t, r, r + 1), stay(r + 1, c), { rows: afterMove(r, r + 1) })
      return
    case 'move-left':
      if (c > 0) reorderCols(host, c, c - 1, stay(r, c - 1))
      return
    case 'move-right':
      if (c < t.align.length - 1) reorderCols(host, c, c + 1, stay(r, c + 1))
      return
    case 'done':
      host.done()
  }
}
