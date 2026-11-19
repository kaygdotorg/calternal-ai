/**
 * Column widths for a table grid: measures the text with a canvas (never the browser's table layout) and decides each
 * column's width in pixels. Pure of the grid's state, so it can be read and tested alone. Split out of `grid.ts`.
 */

import { plainCellText } from './cells'
import type { Tbl } from './model'
import { shrinkToFit } from './shrink'

/** Tables with more columns than this are never squeezed into their card: they scroll, as in the mockup. */
const SHRINK_COLS = 4

/** What `measureColumns` found: the width of each column, its floor, and the width of one `0` (for dragged widths). */
export interface Measured {
  widths: number[]
  minPx: number[]
  chPx: number
}

/**
 * Measures columns of `t` for a card `width` px wide. `userCh` holds the widths a person dragged (in characters, null:
 * the column sizes itself) and `numeric` which columns hold numbers.
 */
export function measureColumns(
  ctx: CanvasRenderingContext2D,
  t: Tbl,
  numeric: boolean[],
  userCh: (number | null)[],
  style: CSSStyleDeclaration,
  width: number,
): Measured {
  const fontSize = parseFloat(style.fontSize) || 15
  const family = style.fontFamily
  const coarse = typeof matchMedia === 'function' && matchMedia('(pointer: coarse)').matches
  const firstPad = (coarse ? 40 : 22) + 12
  ctx.font = `400 ${fontSize}px ${family}`
  const chPx = ctx.measureText('0').width || 8

  const natural = t.align.map((_, c) => {
    let widest = 0
    for (let r = 0; r < t.rows.length; r++) {
      ctx.font = `${r === 0 ? 620 : 400} ${fontSize}px ${family}`
      const text = plainCellText(t.rows[r]?.[c] ?? '')
      if (text) widest = Math.max(widest, ctx.measureText(text).width)
    }
    return Math.ceil(widest * 1.05) + (c === 0 ? firstPad : 26)
  })
  const minPx = t.align.map((_, c) => {
    let widest = 0
    for (let r = 0; r < t.rows.length; r++) {
      ctx.font = `${r === 0 ? 620 : 400} ${fontSize}px ${family}`
      for (const word of plainCellText(t.rows[r]?.[c] ?? '').split(/\s+/)) {
        for (const piece of word.split(/(?<=[-\u2010-\u2014])/))
          if (piece) widest = Math.max(widest, ctx.measureText(piece).width)
      }
    }
    return Math.ceil(widest * 1.06) + 3 + (c === 0 ? firstPad : 26)
  })

  const user = userCh.length === t.align.length ? userCh : []
  const next = natural.map((w, c) => {
    const least = minPx[c] ?? 0
    if (user[c]) return Math.max(least, Math.round(user[c]! * chPx + (c === 0 ? firstPad : 26)))
    if (numeric[c]) return Math.max(w, 64, least)
    const min = Math.max(9.5 * fontSize + (c === 0 ? 14 : 0), least)
    const max = c === 0 ? Math.max(min, width * 0.6) : Math.max(min, 22 * fontSize)
    return Math.min(Math.max(w, min), max)
  })
  // The mockup lets a wide table scroll under its pinned first column. Only a small table (up to SHRINK_COLS columns)
  // that can fit by wrapping text (every column at its floor, the longest word or a number's own width, still fits
  // the card: a three-column table on a phone) wraps to fit, so no column is left behind the fade. A dragged width is
  // a person's choice and stays.
  if (next.length <= SHRINK_COLS && next.reduce((a, b) => a + b, 0) > width) {
    const floor = next.map((w, c) => (user[c] ? w : Math.max(minPx[c] ?? 0, numeric[c] ? 64 : 0)))
    if (floor.reduce((a, b) => a + b, 0) <= width) next.splice(0, next.length, ...shrinkToFit(next, floor, width))
  }
  const sum = next.reduce((a, b) => a + b, 0)
  if (sum < width) {
    const free = t.align.map((_, c) => c).filter((c) => !user[c])
    const text = free.filter((c) => !numeric[c])
    const pool = text.length ? text : user.some(Boolean) ? [] : free
    if (pool.length) {
      const to = text.length ? pool.reduce((best, c) => (next[c]! > next[best]! ? c : best), pool[0]!) : pool[pool.length - 1]!
      next[to] = next[to]! + width - sum
    }
  }
  return { widths: next.map((w) => Math.round(w * 2) / 2), minPx, chPx }
}
