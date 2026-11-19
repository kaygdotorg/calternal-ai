/**
 * The edges of a table: resizing a column or row, and carrying a picked row or column to a new place.
 *
 * Resizing. A mouse drags a column's right border (an 8 px zone; double-click lets the column size itself again) or a
 * row's bottom border (a minimum height, never saved). A finger picks the column or row first and drags the handle that
 * shows on its edge. The grid previews each dragged width in pixels, then keeps the column width in characters (`ch`,
 * the unit the frontmatter holds) after the pointer is released.
 *
 * Carrying. A picked row or column is dragged by its strip; the others make room as it passes them, and it settles into
 * its slot before the order changes.
 */

import type { TableKind } from './state'

export interface EdgeHost {
  readonly dom: HTMLElement
  /** The grid's `<table>`: `rows` lists every row across its row groups. */
  readonly table: HTMLTableElement
  /** The picked row or column (or the cell the caret is in). */
  picked(): { kind: TableKind; r: number; c: number } | null
  /** A column's width was dragged to this many characters (null: it sizes itself). */
  setWidth(c: number, ch: number | null): void
  /** A column's in-progress width, in px, should be shown without measuring its text. */
  previewWidth(c: number, px: number): void
  /** A row's minimum height in px (null: its own). */
  setHeight(r: number, px: number | null): void
  /** A carried row or column has been put down in its new place. */
  reorder(kind: 'row' | 'col', from: number, to: number): void
}

const ease = 'cubic-bezier(0.23, 1, 0.32, 1)'

// ---- Resizing

type Zone = { kind: 'col' | 'row'; i: number }

/** The pad a column's cells add around their text, so a width in characters can be told from one in pixels. */
function padOf(td: HTMLElement): number {
  const cell = td.querySelector('.cell')
  if (!cell) return 24
  const cs = getComputedStyle(cell)
  return (parseFloat(cs.paddingLeft) || 0) + (parseFloat(cs.paddingRight) || 0)
}

/** One character (the width of `0`) in the cell's font, in px. */
function chPx(td: HTMLElement): number {
  const probe = document.createElement('span')
  probe.style.cssText = 'position:absolute;visibility:hidden;width:10ch;height:0'
  td.append(probe)
  const w = probe.getBoundingClientRect().width / 10
  probe.remove()
  return w || 8
}

export function attachResize(h: EdgeHost): void {
  const table = h.table
  let drag: { kind: 'col' | 'row'; i: number; start: number; base: number; id: number; el: HTMLElement; px: number } | null = null

  const cellsOf = (kind: 'col' | 'row', i: number): HTMLElement[] =>
    kind === 'col' ? Array.from(h.table.rows, (r) => r.cells[i]).filter((c): c is HTMLTableCellElement => !!c) : Array.from(h.table.rows[i]?.cells ?? [])

  const zoneAt = (e: MouseEvent): Zone | null => {
    const td = (e.target as Element).closest<HTMLElement>('.td')
    if (!td) return null
    const r = td.getBoundingClientRect()
    if (Math.abs(e.clientX - r.right) <= 4) return { kind: 'col', i: +td.dataset.c! }
    if (Math.abs(e.clientY - r.bottom) <= 3) return { kind: 'row', i: (td.parentElement as HTMLTableRowElement).rowIndex }
    return null
  }

  function begin(kind: 'col' | 'row', i: number, e: PointerEvent, el: HTMLElement) {
    el.setPointerCapture(e.pointerId)
    const td = cellsOf(kind, i)[0]
    const base = kind === 'col' ? td?.offsetWidth ?? 80 : (td?.parentElement?.getBoundingClientRect().height ?? 40)
    drag = { kind, i, start: kind === 'col' ? e.clientX : e.clientY, base, id: e.pointerId, el, px: base }
    h.dom.classList.add('resizing')
  }
  function move(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return
    const d = (drag.kind === 'col' ? e.clientX : e.clientY) - drag.start
    if (drag.kind === 'col') {
      drag.px = Math.max(24, Math.min(640, drag.base + d))
      h.previewWidth(drag.i, drag.px)
    } else {
      drag.px = Math.max(36, Math.min(400, drag.base + d))
      for (const c of cellsOf('row', drag.i)) c.style.height = `${drag.px}px`
    }
  }
  function end(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return
    const d = drag
    drag = null
    h.dom.classList.remove('resizing')
    if (d.px === d.base) return
    if (d.kind === 'row') return h.setHeight(d.i, d.px)
    const head = cellsOf('col', d.i)[0]!
    const actual = head.offsetWidth // what the table allowed: never under the longest word
    h.setWidth(d.i, Math.max(1, Math.ceil((actual - padOf(head)) / chPx(head))))
  }

  // A mouse: the cursor says where a border can be dragged.
  h.dom.addEventListener('pointermove', (e) => {
    if (drag) return move(e)
    if (e.pointerType !== 'mouse') return
    const z = zoneAt(e)?.kind ?? ''
    if ((table.dataset.zone ?? '') !== z) table.dataset.zone = z
  })
  // A second press on the same border within 400 ms lets the column (or row) size itself again.
  let last = { at: 0, kind: '', i: -1 }
  h.dom.addEventListener(
    'pointerdown',
    (e) => {
      if (e.pointerType !== 'mouse' || e.button !== 0) return
      const z = zoneAt(e)
      if (!z || (e.target as Element).closest('.rs, .cs')) return
      e.preventDefault()
      e.stopPropagation()
      const again = e.timeStamp - last.at < 400 && last.kind === z.kind && last.i === z.i
      last = { at: again ? 0 : e.timeStamp, kind: z.kind, i: z.i }
      if (!again) return begin(z.kind, z.i, e, table)
      if (z.kind === 'col') h.setWidth(z.i, null)
      else h.setHeight(z.i, null)
    },
    true,
  )
  h.dom.addEventListener('pointerup', end)
  h.dom.addEventListener('pointercancel', end)
  h.dom.addEventListener('mousedown', (e) => {
    if (table.dataset.zone) e.preventDefault() // a drag on a border never moves the caret
  })
  // A finger's handles (they show on the picked column's or row's edge).
  h.dom.addEventListener('pointerdown', (e) => {
    const handle = (e.target as Element).closest<HTMLElement>('.cz, .trz')
    if (!handle) return
    e.stopPropagation()
    e.preventDefault()
    const td = handle.closest<HTMLElement>('.td')!
    const row = handle.classList.contains('trz')
    begin(row ? 'row' : 'col', row ? (td.parentElement as HTMLTableRowElement).rowIndex : +td.dataset.c!, e, handle)
  })
}

// ---- Carrying a picked row or column

export interface Carry {
  /** Has this pointer been carrying (so the tap that ends it is not a pick)? */
  moved: boolean
}

/** Starts carrying the picked strip's row or column. Returns the carry, or null when the strip is not picked. */
export function startCarry(h: EdgeHost, e: PointerEvent, strip: HTMLElement, kind: 'row' | 'col', from: number): Carry {
  const carry: Carry = { moved: false }
  const rows = Array.from(h.table.rows)
  const lo = kind === 'row' ? 1 : 0
  const size = kind === 'row' ? rows.length : (rows[0]?.cells.length ?? 0)
  const slots = Array.from({ length: size }, (_, j) => {
    const r = (kind === 'row' ? rows[j]! : rows[0]!.cells[j]!).getBoundingClientRect()
    return kind === 'row' ? { a: r.top, size: r.height } : { a: r.left, size: r.width }
  })
  const carried = (j: number): HTMLElement[] => (kind === 'row' ? Array.from(rows[j]!.cells) : rows.map((r) => r.cells[j]!))
  const start = kind === 'row' ? e.clientY : e.clientX
  let to = from
  strip.setPointerCapture(e.pointerId)

  const place = (j: number, shift: number, lifted: boolean) => {
    for (const el of carried(j)) {
      el.style.transition = lifted ? 'none' : `transform 200ms ${ease}`
      el.style.transform = shift ? `${kind === 'row' ? 'translateY' : 'translateX'}(${shift}px)` : ''
      el.classList.toggle('lift', lifted)
    }
  }
  const move = (ev: PointerEvent) => {
    if (ev.pointerId !== e.pointerId) return
    const delta = (kind === 'row' ? ev.clientY : ev.clientX) - start
    if (!carry.moved && Math.abs(delta) < 5) return
    if (!carry.moved) {
      carry.moved = true
      h.dom.classList.add('dragging')
    }
    const me = slots[from]!
    const first = slots[lo]!
    const last = slots[size - 1]!
    const dd = Math.max(first.a - me.a, Math.min(last.a + last.size - (me.a + me.size), delta))
    const centre = me.a + me.size / 2 + dd
    to = lo
    for (let j = lo; j < size; j++) if (centre >= slots[j]!.a + slots[j]!.size / 2) to = j
    for (let j = lo; j < size; j++) {
      const shift = j === from ? dd : to > from && j > from && j <= to ? -me.size : to < from && j >= to && j < from ? me.size : 0
      place(j, shift, j === from)
    }
  }
  const up = (ev: PointerEvent) => {
    if (ev.pointerId !== e.pointerId) return
    strip.removeEventListener('pointermove', move)
    strip.removeEventListener('pointerup', up)
    strip.removeEventListener('pointercancel', up)
    if (!carry.moved) return
    // The carried one slides into its slot, then the order changes (the others are already where they will be).
    const slot = to > from ? slots.slice(from + 1, to + 1).reduce((s, p) => s + p.size, 0) : to < from ? -slots.slice(to, from).reduce((s, p) => s + p.size, 0) : 0
    place(from, slot, true)
    for (const el of carried(from)) el.style.transition = `transform 200ms ${ease}`
    setTimeout(() => {
      for (let j = lo; j < size; j++) for (const el of carried(j)) (el.style.transition = 'none'), (el.style.transform = ''), el.classList.remove('lift')
      h.dom.classList.remove('dragging')
      if (to !== from) h.reorder(kind, from, to)
    }, slot ? 200 : 0)
  }
  strip.addEventListener('pointermove', move)
  strip.addEventListener('pointerup', up)
  strip.addEventListener('pointercancel', up)
  return carry
}
