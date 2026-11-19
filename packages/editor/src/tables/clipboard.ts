/**
 * Copy and paste for a table grid: what a picked row or column puts on the clipboard, and how pasted spreadsheet
 * cells land in the table. Pure of the grid's state. Split out of `grid.ts`.
 */

import { type Tbl, insertCol, insertRow } from './model'
import type { Cursor } from './paint'

/** A picked row or column as tab-separated text and as a small HTML table. */
export function copyData(t: Tbl, cur: Cursor): { text: string; html: string } {
  const part = cur.kind === 'row' ? [t.rows[cur.r] ?? []] : t.rows.map((r) => [r[cur.c] ?? ''])
  const plain = (s: string) => s.replace(/[\t\n]+/g, ' ')
  const esc = (s: string) => plain(s).replace(/&/g, '&amp;').replace(/</g, '&lt;')
  return {
    text: part.map((r) => r.map(plain).join('\t')).join('\n'),
    html: `<table>${part.map((r) => `<tr>${r.map((c) => `<td>${esc(c)}</td>`).join('')}</tr>`).join('')}</table>`,
  }
}

/** `t` with the pasted `grid` of cells put in from row `r0`, column `c0`; the table grows to hold them. */
export function pasteCells(t: Tbl, grid: string[][], r0: number, c0: number): Tbl {
  while (t.rows.length < r0 + grid.length) t = insertRow(t, t.rows.length)
  while (t.align.length < c0 + grid[0]!.length) t = insertCol(t, t.align.length)
  t = { rows: t.rows.map((x) => x.slice()), align: t.align }
  grid.forEach((x, i) => x.forEach((v, j) => (t.rows[r0 + i]![c0 + j] = v)))
  return t
}
