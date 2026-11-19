/**
 * The grid's DOM pieces: how one cell and the table body are built, and the small caret and cell lookups the grid's
 * key and focus handling lean on. No state lives here; everything takes the elements it needs. Split out of `grid.ts`.
 */

import { hasInline, paintInline } from './inline'

export type Cell = HTMLElement
export type Where = 'start' | 'end' | 'all'

/**
 * Rows per `<tbody>`. Each group is skipped by the browser while it is off screen (tables.css `content-visibility`),
 * so a key in a cell lays out, paints and checks the groups near the screen, not every row of a long table. One group
 * per 16 rows keeps that check small (it runs per group, not per row) while a group stays about a screen tall.
 */
const ROW_GROUP = 16

let plain: boolean | undefined
/** Cells take plain text only where the browser can (no bold or pasted styling inside a cell). */
function plainOnly(): boolean {
  if (plain === undefined) {
    const probe = document.createElement('div')
    probe.contentEditable = 'plaintext-only'
    plain = probe.contentEditable === 'plaintext-only'
  }
  return plain
}

export function make<K extends keyof HTMLElementTagNameMap>(tag: K, cls?: string): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag)
  if (cls) el.className = cls
  return el
}

/** A cell's row in the whole table (`rowIndex` counts across the row groups, see `ROW_GROUP`). */
export const rOf = (el: Element): number => (el.closest('tr') as HTMLTableRowElement | null)?.rowIndex ?? 0

/** Removes Markdown marks that do not contribute to the width a person reads. */
export function plainCellText(markdown: string): string {
  return markdown.replace(/\[([^\]]*)\]\(<[^>]*>\)/g, '$1').replace(/[*_`~=]/g, '')
}

/** A cell at rest shows its markdown drawn (bold, links, code); with the caret in it, as the file has it. */
export function show(cell: Cell, value: string, drawn = true): void {
  if (drawn && hasInline(value)) {
    paintInline(cell, value)
    cell.dataset.drawn = ''
  } else {
    cell.textContent = value
    delete cell.dataset.drawn
  }
}

/** The thin edge strips a row or column is picked, dragged or resized by. */
function strip(cls: 'rs' | 'cs' | 'cz' | 'trz'): HTMLElement {
  const i = make('i', cls)
  i.setAttribute('aria-hidden', 'true')
  return i
}

/** Builds the table's row groups (the header row first) for `rows`; read-only tables get no edge strips. */
export function buildRows(rows: string[][], readOnly: boolean): DocumentFragment {
  const frag = document.createDocumentFragment()
  let group: HTMLTableSectionElement = make('tbody')
  rows.forEach((cells, r) => {
    const tr = make('tr')
    tr.setAttribute('role', 'row')
    cells.forEach((value, c) => {
      const td = make(r === 0 ? 'th' : 'td', r === 0 ? 'td th' : 'td')
      td.dataset.c = String(c)
      if (r === 0) {
        td.scope = 'col'
        td.setAttribute('role', 'columnheader')
      } else td.setAttribute('role', 'cell')
      if (!readOnly && c === 0 && r > 0) td.append(strip('rs'))
      if (!readOnly && r === 0) td.append(strip('cs'), strip('cz'))
      if (!readOnly && c === 0 && r > 0) td.append(strip('trz'))
      const cell = make('div', 'cell')
      cell.contentEditable = readOnly ? 'false' : plainOnly() ? 'plaintext-only' : 'true'
      cell.setAttribute('role', 'textbox')
      cell.setAttribute('aria-label', r === 0 ? `Column ${c + 1}` : `Cell, column ${c + 1}`)
      cell.spellcheck = false
      cell.autocapitalize = 'off'
      cell.dataset.c = String(c)
      show(cell, value)
      td.append(cell)
      tr.append(td)
    })
    if (r % ROW_GROUP === 0) frag.append((group = make('tbody')))
    group.append(tr)
  })
  // A short last group's size guess is its own rows, not a full group's (tables.css guesses 46 px a row).
  const tail = rows.length % ROW_GROUP
  const last = frag.lastChild as HTMLTableSectionElement | null
  if (last && tail) last.style.setProperty('contain-intrinsic-block-size', `auto calc(${tail} * var(--trow))`)
  return frag
}

/** The caret sits at the very start (`end` false) or end of the cell's text. */
export function atEdge(el: HTMLElement, end: boolean): boolean {
  const s = getSelection()
  if (!s || !s.rangeCount || !s.isCollapsed || !s.anchorNode) return false
  const r = document.createRange()
  r.selectNodeContents(el)
  if (end) r.setStart(s.anchorNode, s.anchorOffset)
  else r.setEnd(s.anchorNode, s.anchorOffset)
  return r.toString() === ''
}

/** The caret is on the cell's first (dir -1) or last (dir 1) line, so up or down leaves the cell. */
export function onEdgeLine(el: HTMLElement, dir: -1 | 1): boolean {
  const s = getSelection()
  if (!s || !s.rangeCount || !s.isCollapsed) return false
  const rect = s.getRangeAt(0).getClientRects()[0]
  if (!rect || (!rect.height && !rect.top)) return true
  const cs = getComputedStyle(el)
  const lh = parseFloat(cs.lineHeight) || 20
  const box = el.getBoundingClientRect()
  return dir < 0
    ? rect.top - (box.top + (parseFloat(cs.paddingTop) || 0)) < lh * 0.6
    : box.bottom - (parseFloat(cs.paddingBottom) || 0) - rect.bottom < lh * 0.6
}
