/**
 * Marks on a drawn table grid: which cells show the caret, a picked row or column, each column's alignment, and which
 * columns carry a dragged width. They only set classes and data attributes on cells that already exist; the grid
 * decides when. Split out of `grid.ts`.
 */

import { type Tbl, shownAlign } from './model'
import type { TableKind } from './state'

/** The cell, row or column the grid has picked. */
export interface Cursor {
  kind: TableKind
  r: number
  c: number
}

/** Draws `cur` on `table` and returns the elements it marked; pass them back next time so they are cleared first. */
export function paintCursor(table: HTMLTableElement, cur: Cursor | null, painted: HTMLElement[]): HTMLElement[] {
  for (const el of painted) {
    el.classList.remove('cur', 'crow', 'ccol', 'sel')
    delete el.dataset.on
  }
  const marked: HTMLElement[] = []
  if (!cur) return marked
  const mark = (els: Iterable<Element | undefined>, ...cls: string[]) => {
    for (const el of els) if (el instanceof HTMLElement) (el.classList.add(...cls), marked.push(el))
  }
  const rows = Array.from(table.rows)
  const col = (c: number) => rows.map((tr) => tr.cells[c])
  if (cur.kind === 'cell') {
    mark(Array.from(rows[cur.r]?.cells ?? []), 'crow')
    mark(col(cur.c), 'ccol')
    mark([rows[cur.r]?.cells[cur.c]], 'cur')
  } else if (cur.kind === 'row') {
    mark(Array.from(rows[cur.r]?.cells ?? []), 'sel')
    for (const s of rows[cur.r]?.querySelectorAll<HTMLElement>('.rs, .trz') ?? []) (s.dataset.on = ''), marked.push(s)
  } else {
    mark(col(cur.c), 'sel')
    for (const s of table.querySelectorAll<HTMLElement>(`td[data-c="${cur.c}"] :is(.cs, .cz), th[data-c="${cur.c}"] :is(.cs, .cz)`)) (s.dataset.on = ''), marked.push(s)
  }
  return marked
}

/** How each column is drawn: the file's alignment, else right for numbers (T6). */
export function styleCols(table: HTMLTableElement, t: Tbl, numeric: boolean[], cols: number[]): void {
  for (const c of cols) {
    const a = shownAlign(t.align[c] ?? '', !!numeric[c])
    for (const tr of Array.from(table.rows)) {
      const td = tr.cells[c]
      if (!td) continue
      td.dataset.a = a
      if (numeric[c]) td.dataset.num = ''
      else delete td.dataset.num
    }
  }
}

/** Marks user-sized columns and applies the row heights remembered for this grid. */
export function markSizes(table: HTMLTableElement, cols: number, userCh: (number | null)[], rowH: (number | undefined)[]): void {
  if (!table.rows[0]) return
  const user = userCh.length === cols ? userCh : []
  // A table with a width of its own may be wider than its card (and then scrolls); a column without one stays readable.
  if (user.some(Boolean)) table.dataset.sized = ''
  else delete table.dataset.sized
  for (const tr of Array.from(table.rows))
    Array.from(tr.cells).forEach((td, c) => {
      if (user[c]) td.dataset.user = ''
      else delete td.dataset.user
    })
  Array.from(table.rows).forEach((tr, r) => {
    const h = rowH[r]
    for (const td of Array.from(tr.cells)) td.style.height = h ? `${h}px` : ''
  })
}
