/**
 * The grid you type into (T2). The file holds GFM pipe text; on screen it is a card of cells, each its own small
 * text field, with no pipes. A cell change patches its raw row at once and tidies a legacy delimiter once;
 * structure changes write the table as a whole. A table nobody edited is never rewritten.
 *
 * This file is the grid itself: its state, focus, pointer gestures and the glue between the parts. The parts live
 * beside it: `revision.ts` (what it shares with the note), `cells.ts` (building cells), `measure.ts` (column widths),
 * `paint.ts` (marks on cells), `commit.ts` (writing into the note), `keys.ts` (keyboard), `ops.ts` (the bar's
 * buttons), `clipboard.ts` (copy and paste), `edges.ts` (row and column edges) and `motion.ts` (moves).
 *
 *   edge of a row or column   selects the whole row or column (the bar then shows what can be done to it)
 */

import type { EditorView } from '@codemirror/view'
import { type Carry, type EdgeHost, attachResize, startCarry } from './edges'
import { type Cell, type Where, buildRows, make, rOf, show } from './cells'
import { copyData, pasteCells } from './clipboard'
import { leaveTable, writeCellRows, writeStructure } from './commit'
import { type KeyHost, onCellKey, onPickedKey } from './keys'
import { measureColumns } from './measure'
import { type Tbl, deleteRow, fromHTML, fromTSV, insertRow, moveCol, moveRow, numericCols, parseTable, replaceCellLine, tidyDelimiterLine } from './model'
import { moves, playCols, playRows, snapshot } from './motion'
import { type OpHost, runOp } from './ops'
import { type Cursor, markSizes, paintCursor, styleCols } from './paint'
import { type GridRevision, type Hooks, createGridRevision, sameLines, tableCellEdit, watchScrollerWidth } from './revision'
import { type TableOp, setTableSel } from './state'
import type { Widths } from './widths'

// The table widget and the tests import the revision pieces from here.
export { createGridRevision, tableCellEdit, watchScrollerWidth }
export type { GridRevision, Hooks }

/** The editable table view that keeps Markdown edits in sync with its note. */
export class Grid {
  readonly dom: HTMLElement
  private readonly scroller: HTMLElement
  /** The `<table>`; its rows sit in groups of 16 (one `<tbody>` each, see `cells.ts`), `table.rows` lists them all. */
  private readonly table: HTMLTableElement
  private t: Tbl
  private numeric: boolean[] = []
  private cur: Cursor | null = null
  private painted: HTMLElement[] = []
  private dirty = new Set<number>()
  private pendingRows = new Set<number>()
  /** A legacy delimiter row is queued with the first real cell edit, never recomputed per keystroke. */
  private delimiterPatch: string | null = null
  private structureDirty = false
  private last = new Map<number, 1 | -1>()
  private readonly ro: ResizeObserver | null
  private readonly run = (op: TableOp) => this.op(op)
  private drawnReadOnly = false
  /** Widths a person dragged, in characters, by column (null: the column sizes itself). */
  private userCh: Widths = []
  /** Row heights a person dragged, by row: this table's, until its structure changes. Never saved. */
  private rowH: (number | undefined)[] = []
  /** Measured widths are shared by every row so an input only lays out the row it changed. */
  private widthsPx: number[] = []
  /** The longest unbroken text in each column, measured again after structure or cell edits. */
  private minPx: number[] = []
  /** Canvas text measurement avoids asking the browser to lay out every table row while typing. */
  private measureCtx: CanvasRenderingContext2D | null = null
  private chPx = 8
  /**
   * A person has scrolled this table (or the caret made it scroll) in this session. Until then a table always sits at
   * its left edge: a table must never open already scrolled sideways (the first column's width, with the column after
   * it hidden under the pinned one; Safari did that when the table had scroll-snap points).
   */
  private scrolledByUser = false
  /** What the keyboard and the bar's operations ask of this grid (see `keys.ts`, `ops.ts`). */
  private readonly keyHost: KeyHost
  private readonly opHost: OpHost

  /** Builds the Grid from the current raw table revision. */
  constructor(
    readonly view: EditorView,
    private revision: GridRevision,
    private readonly hooks: Hooks,
  ) {
    this.keyHost = {
      view,
      table: () => this.t,
      cursor: () => this.cur,
      go: (r, c, where) => this.go(r, c, where),
      leaveTo: (dir) => this.leaveTo(dir),
      addRow: (at) => this.edit(insertRow(this.t, at)),
      enterKey: (shift, r, c) => this.enterKey(shift, r, c),
      flush: () => this.flush(),
      dropRow: (r, go) => this.edit(deleteRow(this.t, r), go),
      op: (op) => this.op(op),
      pick: (kind, r, c) => this.pick(kind, r, c),
    }
    this.opHost = {
      cursor: () => this.cur,
      table: () => this.t,
      tableRows: () => Array.from(this.table.rows),
      userCh: () => this.userCh,
      last: this.last,
      flush: () => this.flush(),
      go: (r, c, where) => this.go(r, c, where),
      pick: (kind, r, c) => this.pick(kind, r, c),
      saveWidths: () => this.saveWidths(),
      edit: (t, after, mv) => this.edit(t, after, mv),
      done: () => {
        ;(document.activeElement as HTMLElement | null)?.blur?.()
        this.cur = null
        this.paint()
        this.release()
        this.view.focus()
      },
    }
    const text = revision.appliedLines.join('\n')
    this.t = parseTable(text)
    this.prepareDelimiterPatch(revision.lines[1])
    this.setWidths(hooks.widths, false)
    this.dom = make('div', 'tw')
    this.dom.contentEditable = 'false'
    this.dom.tabIndex = -1
    this.scroller = make('div', 'ts')
    const table = (this.table = make('table', 'tg'))
    table.setAttribute('aria-label', 'Table')
    this.scroller.append(table)
    this.dom.append(this.scroller)
    this.render()

    const d = this.dom
    d.addEventListener('focusin', (e) => this.onFocusIn(e))
    d.addEventListener('focusout', (e) => this.onFocusOut(e))
    d.addEventListener('pointerdown', (e) => this.onPointerDown(e))
    d.addEventListener('keydown', (e) => this.onKeyDown(e))
    d.addEventListener('beforeinput', (e) => this.onBeforeInput(e as InputEvent))
    d.addEventListener('input', (e) => this.onInput(e))
    d.addEventListener('paste', (e) => this.onPaste(e))
    d.addEventListener('mousedown', (e) => {
      if ((e.target as Element).closest('.rs, .cs')) e.preventDefault() // the caret stays until the edge is chosen
    })
    d.addEventListener('pointerdown', (e) => this.onEdgeDown(e))
    d.addEventListener('pointerup', (e) => this.onEdgeUp(e))
    d.addEventListener('copy', (e) => this.onCopy(e))
    attachResize(this.host())
    this.scroller.addEventListener('scroll', () => this.looks(), { passive: true })
    // The first thing a person does to the scroller (wheel, finger, mouse) is theirs: from then on the grid never resets it.
    for (const type of ['wheel', 'touchstart', 'pointerdown'])
      this.scroller.addEventListener(type, () => (this.scrolledByUser = true), { once: true, passive: true })
    this.ro = watchScrollerWidth(this.scroller, () => (this.fit(), this.looks()))
  }

  destroy(): void {
    this.ro?.disconnect()
    this.release()
  }

  /** Computes the one-time legacy delimiter patch outside the keystroke path. */
  private prepareDelimiterPatch(line: string | undefined): void {
    this.delimiterPatch = null
    if (line === undefined) return
    const tidy = tidyDelimiterLine(line)
    if (tidy !== line) this.delimiterPatch = tidy
  }

  // ---- Drawing

  private render(): void {
    const { rows, align } = this.t
    this.numeric = numericCols(this.t)
    this.drawnReadOnly = this.view.state.readOnly
    if (this.userCh.length !== align.length) this.userCh = align.map(() => null)
    this.table.replaceChildren(buildRows(rows, this.drawnReadOnly))
    this.styleCols(align.map((_, c) => c))
    this.sizes()
    this.painted = []
    this.paint()
    requestAnimationFrame(() => (this.fit(), this.looks()))
  }

  private sizes(): void {
    markSizes(this.table, this.t.align.length, this.userCh, this.rowH)
  }

  private styleCols(cols: number[]): void {
    styleCols(this.table, this.t, this.numeric, cols)
  }

  /** The widths in the note's frontmatter changed (or are being read for the first time). */
  setWidths(w: Widths | null, redraw = true): void {
    if (w && JSON.stringify(w) === JSON.stringify(this.userCh)) return
    const cols = this.t.align.length
    const next = w && w.length === cols ? w.slice() : Array.from({ length: cols }, () => null)
    if (redraw && JSON.stringify(next) === JSON.stringify(this.userCh)) return
    this.userCh = next
    if (redraw) {
      this.sizes()
      this.fit()
    }
  }

  private saveWidths(): void {
    const { index, count } = this.hooks.where(this.pos())
    this.hooks.save(index, count, this.userCh.some(Boolean) ? this.userCh.slice() : null)
  }

  /** Gives edge gestures the live grid to update while a row or column moves. */
  private host(): EdgeHost {
    return {
      dom: this.dom,
      table: this.table,
      picked: () => this.cur,
      setWidth: (c, ch) => {
        this.userCh[c] = ch
        this.sizes()
        this.fit()
        this.saveWidths()
      },
      previewWidth: (c, px) => this.previewWidth(c, px),
      setHeight: (r, px) => {
        this.rowH[r] = px ?? undefined
        this.sizes()
      },
      reorder: (kind, from, to) => this.reorder(kind, from, to),
    }
  }

  /** A carried row or column put down: it is already where it will be, so nothing is played. */
  private reorder(kind: 'row' | 'col', from: number, to: number): void {
    if (kind === 'row') this.edit(moveRow(this.t, from, to), () => this.pick('row', to, 0))
    else {
      this.userCh.splice(to, 0, this.userCh.splice(from, 1)[0] ?? null)
      this.saveWidths()
      this.edit(moveCol(this.t, from, to), () => this.pick('col', 0, to))
    }
  }

  /** A picked row or column copies as tab-separated text and as a small table. */
  private onCopy(e: ClipboardEvent): void {
    const cur = this.cur
    if (!cur || cur.kind === 'cell' || e.target !== this.dom || !e.clipboardData) return
    const { text, html } = copyData(this.t, cur)
    e.clipboardData.setData('text/plain', text)
    e.clipboardData.setData('text/html', html)
    e.preventDefault()
  }

  private cellAt(r: number, c: number): Cell | null {
    return this.table.rows[r]?.cells[c]?.querySelector<Cell>('.cell') ?? null
  }

  private paint(): void {
    this.painted = paintCursor(this.table, this.cur, this.painted)
  }

  /**
   * Measures columns on load, resize, structure changes, and cell blur. Typing keeps these shared widths fixed, so the
   * browser lays out the changed row without running the table's intrinsic-width algorithm on every key.
   */
  private fit(): void {
    const ctx = (this.measureCtx ??= document.createElement('canvas').getContext('2d'))
    if (!this.table || !ctx || !this.t.align.length) return
    const m = measureColumns(ctx, this.t, this.numeric, this.userCh, getComputedStyle(this.dom), this.scroller.clientWidth)
    this.chPx = m.chPx
    this.minPx = m.minPx
    this.applyWidths(m.widths)
  }

  /** Applies cached column widths without reading cell content or triggering a table-wide measurement. */
  private applyWidths(widths: number[]): void {
    const table = this.table
    if (!table) return
    this.widthsPx = widths
    const total = widths.reduce((a, b) => a + b, 0)
    const columns = widths.map((w) => `${w}px`).join(' ')
    if (table.style.getPropertyValue('--cols') !== columns) table.style.setProperty('--cols', columns)
    if (table.style.getPropertyValue('--tw') !== `${total}px`) table.style.setProperty('--tw', `${total}px`)
    const scroller = this.scroller
    const wide = scroller.scrollWidth > scroller.clientWidth + 1
    // Only a table wider than its card pins its first column (tables.css `[data-wide]`). Read here, where the columns
    // are measured anyway, and never on a key: typing keeps the column widths, so it cannot change.
    if (wide !== (this.dom.dataset.wide !== undefined)) {
      if (wide) this.dom.dataset.wide = ''
      else delete this.dom.dataset.wide
    }
    // An untouched table starts at its left edge.
    if (!this.scrolledByUser && scroller.scrollLeft !== 0) scroller.scrollLeft = 0
    this.looks()
  }

  /** Shows a dragged width immediately; the final width is measured and saved once the drag ends. */
  private previewWidth(c: number, px: number): void {
    if (c < 0 || c >= this.widthsPx.length) return
    const next = this.widthsPx.slice()
    next[c] = Math.max(this.minPx[c] ?? 24, px)
    this.applyWidths(next)
  }

  /** The pinned column's shadow while scrolled, the soft fade while more is off to the right. */
  private looks(): void {
    const s = this.scroller
    const scrolled = s.scrollLeft > 1
    const more = s.scrollLeft < s.scrollWidth - s.clientWidth - 1
    if (this.dom.dataset.scrolled !== String(scrolled)) this.dom.dataset.scrolled = String(scrolled)
    if (this.dom.dataset.more !== String(more)) this.dom.dataset.more = String(more)
  }

  /** The note went live (or read-only) under the grid: draw the cells again, now to type in (or not). */
  refreshEditable(): void {
    if (this.drawnReadOnly !== this.view.state.readOnly) this.render()
  }

  // ---- The text in the document

  /** The document's text for this table changed from outside (undo, another device, source): take it in. */
  adopt(revision: GridRevision): void {
    if (revision === this.revision) return
    const same = sameLines(this.revision.appliedLines, revision.appliedLines)
    this.revision = revision
    this.prepareDelimiterPatch(revision.lines[1])
    this.pendingRows.clear()
    this.structureDirty = false
    if (same) return
    const at = this.active()
    this.t = parseTable(revision.appliedLines.join('\n'))
    this.dirty.clear()
    this.render()
    if (at) this.go(Math.min(at.r, this.t.rows.length - 1), Math.min(at.c, this.t.align.length - 1), 'end')
    else if (this.cur) {
      this.cur = null
      this.release()
    }
  }

  /** Where this table starts in the note: the note's own table record (cheap), else the DOM's place in the view. */
  private pos(): number {
    const known = this.hooks.start?.(this.revision)
    if (known != null && known >= 0) return known
    try {
      return this.view.posAtDOM(this.dom)
    } catch {
      return -1
    }
  }

  /** Writes anything not yet in the note (a structure edit); typed cell text never waits, see `onInput`. */
  private flush(): void {
    if (this.structureDirty || this.pendingRows.size) this.commit()
  }

  /** Applies either the pending raw-row patches or the explicit structural table edit. */
  private commit(): void {
    const from = this.pos()
    if (from < 0) return
    if (this.structureDirty) {
      const settle = () => {
        this.structureDirty = false
        this.pendingRows.clear()
      }
      if (!writeStructure(this.view, this.revision, this.t, from, settle)) this.adoptCurrentText(from)
      return
    }
    if (!this.pendingRows.size) return
    if (!writeCellRows(this.view, this.revision, this.pendingRows, from)) return this.adoptCurrentText(from)
    this.pendingRows.clear()
  }

  /** Reads the current table only after a row-level check shows that the Grid revision is stale. */
  private adoptCurrentText(from: number): void {
    const to = Math.min(this.view.state.doc.length, from + this.revision.textLength)
    this.adopt(createGridRevision(this.view.state.sliceDoc(from, to)))
  }

  // ---- Cells

  private active(): { r: number; c: number } | null {
    const a = document.activeElement as HTMLElement | null
    return a && this.dom.contains(a) && a.classList.contains('cell') ? { r: rOf(a), c: +a.dataset.c! } : null
  }

  private put(el: Cell, where: Where): void {
    el.focus({ preventScroll: true })
    const s = getSelection()
    if (!s) return
    const rg = document.createRange()
    rg.selectNodeContents(el)
    if (where !== 'all') rg.collapse(where === 'start')
    s.removeAllRanges()
    s.addRange(rg)
    this.reveal(el)
  }

  /** Only two things scroll for a caret: the page up and down, and this table sideways. */
  private reveal(el: Cell): void {
    const page = this.view.scrollDOM
    const r = el.getBoundingClientRect()
    const pr = page.getBoundingClientRect()
    const top = Math.max(pr.top, 0) + 76
    const bottom = Math.min(pr.bottom, innerHeight) - 96
    if (r.top < top) page.scrollTop -= top - r.top
    else if (r.bottom > bottom) page.scrollTop += r.bottom - bottom
    const sc = this.scroller
    const td = el.closest<HTMLElement>('.td')!
    const pin = this.table.rows[0]?.cells[0]?.offsetWidth ?? 0
    const sr = sc.getBoundingClientRect()
    const tr = td.getBoundingClientRect()
    if (+td.dataset.c! > 0 && tr.left < sr.left + pin) sc.scrollLeft -= sr.left + pin - tr.left
    else if (tr.right > sr.right) sc.scrollLeft += tr.right - sr.right
    else return
    this.scrolledByUser = true
  }

  /** Puts the caret in a cell (clamped to the table). Public: the widget uses it for a new table. */
  go(r: number, c: number, where: Where = 'all'): void {
    const el = this.cellAt(Math.max(0, Math.min(r, this.t.rows.length - 1)), Math.max(0, Math.min(c, this.t.align.length - 1)))
    if (el) this.put(el, where)
  }

  /** Out of the table to the line above or below (a new line after the last one). */
  private leaveTo(dir: -1 | 1): void {
    this.flush()
    const from = this.pos()
    if (from < 0) return
    leaveTable(this.view, from, from + this.revision.textLength, dir)
  }

  // ---- Focus

  private publish(): void {
    const cur = this.cur
    if (!cur) return
    const c = Math.min(cur.c, this.t.align.length - 1)
    setTableSel(
      {
        kind: cur.kind,
        r: cur.r,
        c,
        rows: this.t.rows.length,
        cols: this.t.align.length,
        align: this.t.align[c] ?? '',
        numeric: !!this.numeric[c],
        sortNext: this.last.get(c) === 1 ? -1 : 1,
      },
      this.run,
    )
  }

  private release(): void {
    setTableSel(null, this.run)
  }

  /** Keeps the first click on drawn Markdown from placing a caret before its closing syntax. */
  private onPointerDown(e: PointerEvent): void {
    if (e.button !== 0 || this.view.state.readOnly || !(e.target instanceof Element)) return
    const cell = e.target.closest<Cell>('.cell')
    if (!cell || cell.dataset.drawn === undefined) return
    e.preventDefault()
    this.go(rOf(cell), +cell.dataset.c!, 'end')
  }

  private onFocusIn(e: FocusEvent): void {
    const el = e.target as HTMLElement
    if (!el.classList.contains('cell')) return
    this.cur = { kind: 'cell', r: rOf(el), c: +el.dataset.c! }
    // A cell with formatting shows its markdown for typing; the caret goes to its end.
    if (el.dataset.drawn !== undefined) {
      show(el, this.t.rows[this.cur.r]?.[this.cur.c] ?? '', false)
      this.put(el, 'end')
    }
    this.paint()
    this.publish()
  }

  private onFocusOut(e: FocusEvent): void {
    const el = e.target as HTMLElement
    if (el.classList.contains('cell') && this.dirty.size) {
      // Leaving a cell whose text changed: its column is looked at again (numbers sit on the right).
      const cols = [...this.dirty]
      this.dirty.clear()
      this.numeric = numericCols(this.t)
      this.styleCols(cols)
      this.fit()
      this.publish()
    }
    this.flush()
    // Not typed in any more: the cell's markdown is drawn again.
    if (el.classList.contains('cell') && el.isConnected) show(el, this.t.rows[rOf(el)]?.[+el.dataset.c!] ?? '')
    if (!document.hasFocus()) return
    const to = e.relatedTarget as Node | null
    if (to && this.dom.contains(to)) return
    queueMicrotask(() => {
      // A row taken away under the caret is not the caret leaving: it is put back in a moment.
      if (!el.isConnected || this.dom.contains(document.activeElement)) return
      if (this.cur) {
        this.cur = null
        this.paint()
        this.release()
      }
    })
  }

  // ---- Keys

  private enterKey(shift: boolean, r: number, c: number): void {
    if (shift) return void (r > 0 && this.go(r - 1, c))
    if (r < this.t.rows.length - 1) return this.go(r + 1, c)
    this.edit(insertRow(this.t, this.t.rows.length))
    this.go(this.t.rows.length - 1, c)
  }

  private onKeyDown(e: KeyboardEvent): void {
    if (e.isComposing) return
    const t = e.target as HTMLElement
    if (t === this.dom) return onPickedKey(e, this.keyHost)
    if (t.classList.contains('cell')) onCellKey(e, t, this.keyHost)
  }

  private onBeforeInput(e: InputEvent): void {
    if (this.view.state.readOnly) return void e.preventDefault()
    if (e.inputType !== 'insertParagraph' && e.inputType !== 'insertLineBreak') return
    e.preventDefault()
    const t = e.target as HTMLElement
    if (t.classList.contains('cell')) this.enterKey(false, rOf(t), +t.dataset.c!)
  }

  /** Keeps the visible cell and raw row in sync, queueing a legacy delimiter tidy only on the first real edit. */
  private onInput(e: Event): void {
    const el = e.target as Cell
    if (!el.classList?.contains('cell')) return
    e.stopPropagation()
    const value = (el.textContent ?? '').replace(/\s*\n\s*/g, ' ')
    const r = rOf(el)
    const c = +el.dataset.c!
    if (this.t.rows[r]?.[c] === value) return
    if (this.delimiterPatch !== null) {
      this.revision.lines[1] = this.delimiterPatch
      this.pendingRows.add(1)
      this.delimiterPatch = null
    }
    this.t.rows[r]![c] = value
    this.dirty.add(c)
    const line = r === 0 ? 0 : r + 1
    this.revision.lines[line] = replaceCellLine(this.revision.lines[line]!, c, value)
    this.revision.version++
    this.pendingRows.add(line)
    // The key is an edit of the note itself, made right here in the same event: no timer, no frame, no copy of the
    // cell's text held back. The cell's DOM is what the browser just typed into; the note's text is made to equal it now.
    this.commit()
  }

  /** A spreadsheet's cells land in cells, from the caret's cell on; anything else goes in as plain text. */
  private onPaste(e: ClipboardEvent): void {
    const el = e.target as HTMLElement
    if (!el.classList?.contains('cell')) return
    e.preventDefault()
    e.stopPropagation()
    const html = e.clipboardData?.getData('text/html') ?? ''
    const text = e.clipboardData?.getData('text/plain') ?? ''
    const grid = (html && fromHTML(html)) || fromTSV(text)
    if (grid && (grid.length > 1 || grid[0]!.length > 1)) {
      const r0 = rOf(el)
      const c0 = +el.dataset.c!
      this.edit(pasteCells(this.t, grid, r0, c0), () => this.go(r0, c0, 'end'))
      return
    }
    document.execCommand('insertText', false, text.replace(/\s*\n\s*/g, ' ').trim())
  }

  // ---- Picking a whole row or column by its edge

  private tap: { x: number; y: number; kind: 'row' | 'col'; i: number } | null = null
  private carry: Carry | null = null

  private onEdgeDown(e: PointerEvent): void {
    const strip = (e.target as Element).closest<HTMLElement>('.rs, .cs')
    if (!strip || (e.pointerType === 'mouse' && e.button !== 0)) return void (this.tap = null)
    const row = strip.classList.contains('rs')
    const td = strip.closest<HTMLElement>('.td')!
    const kind = row ? 'row' : 'col'
    const i = row ? rOf(td) : +td.dataset.c!
    this.tap = { x: e.clientX, y: e.clientY, kind, i }
    // Already picked: the strip carries it (a tap that does not move still goes on to the tap below).
    const same = this.cur?.kind === kind && (row ? this.cur.r : this.cur.c) === i
    if (same && !this.view.state.readOnly) this.carry = startCarry(this.host(), e, strip, kind, i)
    // A mouse picks at once; a finger waits to be sure it is a tap, not a scroll.
    if (e.pointerType === 'mouse' && !same) this.onEdgeUp(e)
  }

  private onEdgeUp(e: PointerEvent): void {
    const tap = this.tap
    this.tap = null
    const carried = this.carry?.moved
    this.carry = null
    if (carried) return
    if (!tap || Math.hypot(e.clientX - tap.x, e.clientY - tap.y) > 8) return
    const same = this.cur?.kind === tap.kind && (tap.kind === 'row' ? this.cur.r : this.cur.c) === tap.i
    // A second tap on a picked column goes in to type.
    if (same && tap.kind === 'col') return this.go(0, tap.i, 'end')
    this.pick(tap.kind, tap.kind === 'row' ? tap.i : 0, tap.kind === 'col' ? tap.i : 0)
  }

  private pick(kind: 'row' | 'col', r: number, c: number): void {
    ;(document.activeElement as HTMLElement | null)?.blur?.()
    getSelection()?.removeAllRanges()
    this.cur = { kind, r, c }
    this.paint()
    this.publish()
    this.dom.focus({ preventScroll: true })
  }

  // ---- Structure (the bar's buttons, and a few keys)

  /** `mv` says where each new row or column came from, so a pointer's change can be played (motion.ts). */
  private edit(t: Tbl, after?: () => void, mv?: { rows?: (i: number) => number; cols?: (c: number) => number }): void {
    const before = mv && moves() ? snapshot(this.table) : null
    this.t = t
    this.dirty.clear()
    this.structureDirty = true
    if (mv?.rows) this.rowH = []
    this.render()
    this.commit() // a structure change is one step of its own
    after?.()
    this.publish()
    if (before && mv?.rows) playRows(this.table, before, mv.rows)
    if (before && mv?.cols) playCols(this.table, before, mv.cols)
  }

  private op(op: TableOp): void {
    runOp(op, this.opHost)
  }
}
