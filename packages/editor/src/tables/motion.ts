/**
 * How a table's structure changes are played (Emil's rules, TASTE "Springy, physical, in place"): what was on screen
 * slides from where it was to where it is, a new row or column eases in, a leaving one fades. Each animation reads
 * its rects once before the change and once after, then runs on the compositor (transform and opacity only). Under
 * 300 ms, ease-out, nothing ever grows from nothing (no `scale(0)`). A key never animates (TASTE: keyboard actions
 * never animate), and neither does reduced motion.
 */

const EASE = 'cubic-bezier(0.23, 1, 0.32, 1)'

let byKey = false
if (typeof addEventListener === 'function') {
  addEventListener('keydown', () => (byKey = true), true)
  addEventListener('pointerdown', () => (byKey = false), true)
}

/** Motion is played for a pointer's change, unless the person asked for less. */
export const moves = (): boolean => !byKey && !matchMedia('(prefers-reduced-motion: reduce)').matches

export type Before = { rows: DOMRect[]; cells: DOMRect[][] }

/** Where every row and cell is now. */
export function snapshot(table: HTMLTableElement): Before {
  const rows = Array.from(table.rows)
  return { rows: rows.map((r) => r.getBoundingClientRect()), cells: rows.map((r) => Array.from(r.cells, (c) => c.getBoundingClientRect())) }
}

const arrive = (els: Element[], dx: number, dy: number) => {
  for (const el of els) el.animate([{ opacity: 0, transform: `translate(${dx}px, ${dy}px)` }, { opacity: 1, transform: 'none' }], { duration: 220, easing: EASE })
}

/** After the change: rows that were somewhere slide from there; `from(i)` is a new row's old index, or -1 if it is new. */
export function playRows(table: HTMLTableElement, before: Before, from: (i: number) => number): void {
  Array.from(table.rows).forEach((tr, i) => {
    const o = from(i)
    const cells = Array.from(tr.cells)
    if (o < 0 || !before.rows[o]) return arrive(cells, 0, -8)
    const dy = before.rows[o].top - tr.getBoundingClientRect().top
    if (Math.abs(dy) >= 1) for (const c of cells) c.animate([{ transform: `translateY(${dy}px)` }, { transform: 'none' }], { duration: 260, easing: EASE })
  })
}

/** The same for columns: `from(c)` is a new column's old index, or -1. */
export function playCols(table: HTMLTableElement, before: Before, from: (c: number) => number): void {
  Array.from(table.rows).forEach((tr, r) => {
    Array.from(tr.cells).forEach((td, c) => {
      const o = from(c)
      const was = before.cells[r]?.[o]
      if (o < 0 || !was) return arrive([td], -8, 0)
      const dx = was.left - td.getBoundingClientRect().left
      if (Math.abs(dx) >= 1) td.animate([{ transform: `translateX(${dx}px)` }, { transform: 'none' }], { duration: 260, easing: EASE })
    })
  })
}

/** A row or column leaving: it fades, then the rest closes up. Resolves when the fade is done. */
export function playOut(cells: Element[]): Promise<unknown> {
  return Promise.all(cells.map((c) => c.animate([{ opacity: 1 }, { opacity: 0 }], { duration: 120, easing: 'ease-out', fill: 'forwards' }).finished.catch(() => {})))
}

/** The old index of new row/column `i` after moving `from` to `to`. */
export const afterMove = (from: number, to: number) => (i: number): number => (i === to ? from : from < to && i >= from && i < to ? i + 1 : to < from && i > to && i <= from ? i - 1 : i)

/** The old index of new row/column `i` after inserting one at `at`. */
export const afterInsert = (at: number) => (i: number): number => (i < at ? i : i === at ? -1 : i - 1)

/** The old index of new row/column `i` after taking `at` away. */
export const afterRemove = (at: number) => (i: number): number => (i < at ? i : i + 1)
