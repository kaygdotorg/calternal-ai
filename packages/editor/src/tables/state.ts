/**
 * What the table in use looks like to the bar (T4). The grid writes it as the caret or a whole row or column
 * moves; the bar reads it and asks the grid to do things through `runTableOp`. Plain data and two functions,
 * so the bar needs no knowledge of how the grid is drawn.
 */

export type TableKind = 'cell' | 'row' | 'col'

export type TableOp =
  | 'row-above'
  | 'row-below'
  | 'col-left'
  | 'col-right'
  | 'row-del'
  | 'col-del'
  | 'align-l'
  | 'align-c'
  | 'align-r'
  | 'sort-asc'
  | 'sort-desc'
  | 'move-up'
  | 'move-down'
  | 'move-left'
  | 'move-right'
  | 'done'

export interface TableSel {
  kind: TableKind
  r: number
  c: number
  rows: number
  cols: number
  /** The file's alignment of this column: '' is `---`. */
  align: '' | 'l' | 'c' | 'r'
  /** The column is all numbers: sort says 1 to 9, and it is drawn right-aligned. */
  numeric: boolean
  /** Which way sorting this column goes next: 1 is A to Z or 1 to 9. */
  sortNext: 1 | -1
}

let current: TableSel | null = null
let runner: ((op: TableOp) => void) | null = null
const listeners = new Set<(s: TableSel | null) => void>()

export const tableSel = (): TableSel | null => current

/** Hears every change of the table in use (null: none). Returns the way to stop listening. */
export function onTableSel(f: (s: TableSel | null) => void): () => void {
  listeners.add(f)
  return () => listeners.delete(f)
}

/** The grid in use says what it is on. `run` is how the bar's buttons reach it. */
export function setTableSel(s: TableSel | null, run?: (op: TableOp) => void): void {
  if (s === null && run && runner !== run) return // a grid that is not the one in use letting go
  current = s
  runner = s ? (run ?? runner) : null
  for (const f of listeners) f(s)
}

export function runTableOp(op: TableOp): void {
  runner?.(op)
}
