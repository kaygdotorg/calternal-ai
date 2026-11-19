/**
 * Tables in the note (T1 to T3). The file keeps plain GFM pipe tables and nothing else. In the default mode each
 * one is drawn as a grid you type into (grid.ts), in place of its pipe text; Show source draws the pipes.
 * Making one: Insert (and /table), a line typed like `| a | b |` then Enter, or a paste from a spreadsheet or page.
 */

import { syntaxTree, syntaxTreeAvailable } from '@codemirror/language'
import { type ChangeDesc, type EditorState, type Extension, type Line, Prec, StateField, Transaction } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, WidgetType, keymap } from '@codemirror/view'
import type { SyntaxNode } from '@lezer/common'
import { frontmatterEnd } from '../syntax/frontmatter'
import { treeTo } from '../syntax/tree-to'
import { createGridRevision, Grid, tableCellEdit, type GridRevision, type Hooks } from './grid'
import { type Widths, readWidths, writeWidths } from './widths'
import { type Tbl, emptyTable, fromHTML, fromTSV, pipeLine, serializeTable, tidyDelimiterLine } from './model'

type Range = { from: number; to: number }
type Held = HTMLElement & { __grid?: Grid }

export { onTableSel, runTableOp, tableSel } from './state'
export type { TableKind, TableOp, TableSel } from './state'

// ---- Finding tables

/**
 * The tables at the top level of the note's body: whole lines, from the header to the last row. Only as far as the syntax
 * tree reaches: opening a long note parses what is drawn first (`syntax/parse-ahead.ts`), and the field looks again each
 * time the tree grows until it is complete, so a table further down gets its grid before a scroll shows it (plan 1c).
 * A table the tree ends inside is left as pending text until the tree has all of it.
 */
function findTables(state: EditorState, upto = 0): { ranges: Range[]; complete: boolean; reached: number } {
  // After an edit: at least as far as the tree reached before (see `treeTo`).
  const tree = treeTo(state, upto)
  const body = frontmatterEnd(state)
  const ranges: Range[] = []
  const complete = tree.length >= state.doc.length || syntaxTreeAvailable(state)
  const c = tree.cursor()
  if (c.firstChild()) {
    do {
      if (c.name !== 'Table' || c.from < body) continue
      // Where an unfinished tree stops (at the start of the line after the last one it parsed), its last table may
      // be cut short: it waits for the rest of its rows.
      if (!complete && c.to + 1 >= tree.length) continue
      const a = state.doc.lineAt(c.from)
      const b = state.doc.lineAt(c.to)
      if (a.from === c.from && b.to === c.to) ranges.push({ from: c.from, to: c.to })
    } while (c.nextSibling())
  }
  return { ranges, complete, reached: tree.length }
}

/** The known table decorations and their Grid identities in one editor state. */
interface Found {
  set: DecorationSet
  count: number
  docLength: number
  complete: boolean
  /** How far the syntax tree reached when the tables were found. */
  reached: number
}

/** Maps CodeMirror's decoration tree after a change without walking every table record. */
function mapFound(value: Found, changes: ChangeDesc, docLength: number): Found {
  return { set: value.set.map(changes), count: value.count, docLength, complete: value.complete, reached: changes.mapPos(value.reached, -1) }
}

/** Reuses mapped widget identities when a full table redraw is needed. */
function previousRevisions(previous?: Found, changes?: ChangeDesc): Map<number, GridRevision> {
  const byFrom = new Map<number, GridRevision>()
  if (!previous) return byFrom
  previous.set.between(0, previous.docLength, (from, _to, decoration) => {
    const revision = (decoration.spec.widget as { revision?: GridRevision } | undefined)?.revision
    if (revision) byFrom.set(changes ? changes.mapPos(from, 1) : from, revision)
  })
  return byFrom
}

/** Reuses the unchanged Grid identity when a table redraw leaves its raw Markdown intact. */
function revisionAt(state: EditorState, range: Range, previous: Map<number, GridRevision>): GridRevision {
  const text = state.sliceDoc(range.from, range.to)
  const revision = previous.get(range.from)
  if (revision && text === revision.appliedLines.join('\n')) return revision
  return createGridRevision(text)
}

/** Finds table blocks once and keeps their raw revisions beside the replacement decorations. */
function draw(state: EditorState, previous?: Found, changes?: ChangeDesc): Found {
  const { ranges, complete, reached } = findTables(state, previous ? (changes ? changes.mapPos(previous.reached, -1) : previous.reached) : 0)
  const widths = readWidths(state)
  const old = previousRevisions(previous, changes)
  const set = Decoration.set(
    ranges.map((r, i) => {
      const revision = revisionAt(state, r, old)
      return Decoration.replace({
        widget: new TableWidget(state.sliceDoc(r.from, r.to), r.from, state.readOnly, widths[i] ?? null, revision),
        block: true,
      }).range(r.from, r.to)
    }),
  )
  return { set, count: ranges.length, docLength: state.doc.length, complete, reached }
}

/** Accepts a cell row patch and its optional one-time delimiter tidy, then maps the existing widget and decoration. */
function cellEdit(value: Found, tr: Transaction): Found | null {
  const edit = tr.annotation(tableCellEdit)
  if (!edit || edit.version !== edit.revision.version || edit.version <= edit.revision.appliedVersion) return null
  const match: { range?: Range } = {}
  value.set.between(edit.from, edit.from + 1, (from, to, decoration) => {
    const revision = (decoration.spec.widget as { revision?: GridRevision } | undefined)?.revision
    if (revision === edit.revision) match.range = { from, to }
  })
  const range = match.range
  if (!range || range.from !== edit.from) return null
  const start = tr.startState.doc.lineAt(range.from)
  if (start.from !== range.from) return null
  const updates: { index: number; insert: string; delta: number }[] = []
  let valid = true
  tr.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
    if (!valid) return
    const line = tr.startState.doc.lineAt(from)
    const index = line.number - start.number
    const expected = edit.revision.appliedLines[index]
    const next = edit.revision.lines[index]
    const text = inserted.toString()
    const safeDelimiterTidy = index === 1 && expected !== undefined && tidyDelimiterLine(expected) === text
    if (
      index < 0 ||
      index >= edit.revision.lines.length ||
      (index === 1 && !safeDelimiterTidy) ||
      line.from !== from ||
      line.to !== to ||
      line.text !== expected ||
      text !== next ||
      text.includes('\n')
    ) {
      valid = false
      return
    }
    updates.push({ index, insert: text, delta: text.length - line.text.length })
  })
  if (!valid || !updates.length) return null
  for (const update of updates) edit.revision.appliedLines[update.index] = update.insert
  edit.revision.appliedVersion = edit.version
  edit.revision.textLength += updates.reduce((total, update) => total + update.delta, 0)
  return mapFound(value, tr.changes, tr.state.doc.length)
}

/** Only a change that could make, break or reach a table looks for tables again: typing elsewhere costs a map. */
const found = StateField.define<Found>({
  create: draw,
  update(value, tr) {
    // A note shown as a preview is read-only and goes live in place: its grids are drawn again, now to type in.
    if (tr.state.readOnly !== tr.startState.readOnly) return draw(tr.state, value)
    if (!tr.docChanged) {
      return !value.complete && syntaxTree(tr.state) !== syntaxTree(tr.startState) ? draw(tr.state, value) : value
    }
    const mappedCellEdit = cellEdit(value, tr)
    if (mappedCellEdit) return mappedCellEdit
    let again = false
    tr.changes.iterChanges((fa, ta, _fb, _tb, inserted) => {
      if (again) return
      if (inserted.length && inserted.toString().includes('|')) again = true
      else if (ta > fa && tr.startState.sliceDoc(fa, ta).includes('|')) again = true
      else value.set.between(Math.max(0, fa - 1), ta + 1, () => void (again = true))
    })
    // The widths a person dragged live in the frontmatter: a change there redraws the tables that have them.
    if (!again && value.count && tr.changes.touchesRange(0, Math.max(frontmatterEnd(tr.startState), 3))) again = true
    if (again) return draw(tr.state, value, tr.changes)
    return mapFound(value, tr.changes, tr.state.doc.length)
  },
  provide: (f) => [EditorView.decorations.from(f, (v) => v.set), EditorView.atomicRanges.of((view) => view.state.field(f).set)],
})

/** The current table positions, derived on demand instead of maintained on every cell input. */
export function tableRanges(state: EditorState): Range[] {
  const value = state.field(found, false)
  if (!value) return []
  const ranges: Range[] = []
  value.set.between(0, state.doc.length, (from, to) => {
    ranges.push({ from, to })
  })
  return ranges
}

// ---- The widget

/** Where the caret goes when a new table shows up: read once when its grid is made. */
let wanted: { from: number; r: number; c: number } | null = null

/** Replaces one pipe-table range with its grid while keeping the current raw-row revision. */
class TableWidget extends WidgetType {
  /** Binds the discovered text range to the current table revision and its lazy Grid. */
  constructor(
    readonly text: string,
    readonly from: number,
    readonly readOnly: boolean,
    readonly widths: Widths | null,
    readonly revision: GridRevision,
  ) {
    super()
  }
  /** Retains the existing DOM only while its raw table revision and visible state still match. */
  override eq(other: TableWidget): boolean {
    return other.text === this.text && other.readOnly === this.readOnly && other.revision === this.revision && JSON.stringify(other.widths) === JSON.stringify(this.widths)
  }
  /** Creates the Grid DOM for this table after the lazy table extension loads. */
  toDOM(view: EditorView): HTMLElement {
    const grid = new Grid(view, this.revision, hooksFor(view, this.widths))
    const dom = grid.dom as Held
    dom.__grid = grid
    const want = wanted
    if (want && want.from === this.from) {
      wanted = null
      // After the Insert sheet has handed focus back (it closes on its own spring), the caret goes in the cell.
      const put = () => {
        if (!document.activeElement?.classList.contains('cell')) grid.go(want.r, want.c, 'all')
      }
      requestAnimationFrame(put)
      setTimeout(put, 120)
      setTimeout(put, 420)
    }
    return dom
  }
  /** Reconciles a live Grid when an outside edit replaces its table revision. */
  override updateDOM(dom: HTMLElement, view: EditorView): boolean {
    const grid = (dom as Held).__grid
    if (!grid || grid.view !== view) return false
    grid.adopt(this.revision)
    grid.setWidths(this.widths)
    grid.refreshEditable()
    return true
  }
  override destroy(dom: HTMLElement): void {
    ;(dom as Held).__grid?.destroy()
  }
  override ignoreEvent(): boolean {
    return true
  }
  /** Reads the revision's row count so measurement stays constant-time during cell edits. */
  override get estimatedHeight(): number {
    return 46 * (this.revision.lines.length - 1)
  }
}

/** Where the table with this raw revision starts, read from the note's table record (no walk of the page's DOM). */
function startOf(state: EditorState, revision: GridRevision): number {
  const value = state.field(found, false)
  let at = -1
  value?.set.between(0, state.doc.length, (from, _to, decoration) => {
    if ((decoration.spec.widget as { revision?: GridRevision } | undefined)?.revision === revision) {
      at = from
      return false
    }
  })
  return at
}

/** What a grid asks of the note: which table it is among the note's, and where the widths are kept (frontmatter). */
function hooksFor(view: EditorView, widths: Widths | null): Hooks {
  return {
    widths,
    where: (pos) => {
      const all = tableRanges(view.state)
      return { index: Math.max(0, all.findIndex((r) => r.from === pos)), count: all.length }
    },
    save: (index, count, w) => writeWidths(view, index, count, w),
    start: (revision) => startOf(view.state, revision),
  }
}

function gridAt(view: EditorView, from: number): Grid | null {
  for (const el of view.dom.querySelectorAll<Held>('.tw')) {
    try {
      if (el.__grid && view.posAtDOM(el) === from) return el.__grid
    } catch {
      /* not in this view */
    }
  }
  return null
}

// ---- Making tables

/**
 * Puts a table in the note: in place of the line (an empty one, or one typed like `| a | b |`), else after it with a
 * blank line between. A blank line always follows, or the next line would read as one more row.
 */
function place(view: EditorView, line: Line, replace: boolean, t: Tbl, focus: { r: number; c: number }): void {
  const doc = view.state.doc
  const prevBlank = line.number === 1 || !doc.line(line.number - 1).text.trim()
  const next = line.number < doc.lines ? doc.line(line.number + 1) : null
  const lead = replace ? (prevBlank ? '' : '\n') : '\n\n'
  const tail = next && next.text.trim() ? '\n' : ''
  const from = replace ? line.from : line.to
  const start = from + lead.length
  wanted = { from: start, ...focus }
  view.dispatch({
    changes: { from, to: line.to, insert: lead + serializeTable(t) + tail },
    selection: { anchor: start },
    scrollIntoView: true,
    userEvent: 'input.table',
  })
}

/** Insert and /table: a header and two empty rows of three columns, the caret in the first header cell (T3). */
export function insertTable(view: EditorView): void {
  const { state } = view
  const line = state.doc.lineAt(state.selection.main.head)
  const empty = !line.text.trim() && line.from >= frontmatterEnd(state)
  place(view, line, empty, emptyTable(3, 2), { r: 0, c: 0 })
}

const inCode = (state: EditorState, pos: number): boolean => {
  for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1); n; n = n.parent)
    if (/^(FencedCode|CodeBlock|InlineCode|CodeText)$/.test(n.name)) return true
  return false
}

/** Enter at the end of a line typed like `| a | b |`: it becomes the table's header, with two empty rows under it. */
export function typedLine(view: EditorView): boolean {
  const { state } = view
  const sel = state.selection.main
  if (!sel.empty) return false
  const line = state.doc.lineAt(sel.head)
  if (sel.head !== line.to || line.from < frontmatterEnd(state)) return false
  const cells = pipeLine(line.text)
  if (!cells || inCode(state, line.from) || tableRanges(state).some((r) => r.from <= line.from && r.to >= line.to)) return false
  place(view, line, true, emptyTable(cells.length, 2, cells), { r: 1, c: 0 })
  return true
}

/** A paste from a spreadsheet or a web page, at the caret in plain text. */
export function pasteRows(view: EditorView, html: string, text: string): boolean {
  const sel = view.state.selection.main
  if (!sel.empty || !view.state.facet(EditorView.editable) || inCode(view.state, sel.head)) return false
  const rows = (html && fromHTML(html)) || fromTSV(text)
  const head = rows?.[0]
  if (!rows || !head || head.length < 2) return false
  const line = view.state.doc.lineAt(sel.head)
  place(view, line, !line.text.trim(), { rows, align: head.map(() => '') }, { r: Math.min(1, rows.length - 1), c: 0 })
  return true
}

// ---- Walking in from the text around a table

function nextTo(view: EditorView, key: 'ArrowDown' | 'ArrowUp' | 'ArrowRight' | 'ArrowLeft'): boolean {
  const { state } = view
  const sel = state.selection.main
  if (!sel.empty) return false
  const line = state.doc.lineAt(sel.head)
  const down = key === 'ArrowDown' || key === 'ArrowRight'
  const t = tableRanges(state).find((r) => (down ? r.from === line.to + 1 : r.to + 1 === line.from))
  if (!t) return false
  if (key === 'ArrowDown' && view.moveVertically(sel, true).head <= line.to) return false
  if (key === 'ArrowUp' && view.moveVertically(sel, false).head >= line.from) return false
  if (key === 'ArrowRight' && sel.head !== line.to) return false
  if (key === 'ArrowLeft' && sel.head !== line.from) return false
  const grid = gridAt(view, t.from)
  if (!grid) return false
  if (down) grid.go(0, 0, 'start')
  else grid.go(Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER, 'end')
  return true
}

const arrows = (['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft'] as const).map((key) => ({ key, run: (v: EditorView) => nextTo(v, key) }))

/** Tables drawn as grids (the default mode). */
export const tables: Extension = [
  found,
  Prec.high(keymap.of([{ key: 'Enter', run: typedLine }, ...arrows])),
  EditorView.domEventHandlers({
    paste(e, view) {
      const d = e.clipboardData
      if (!d || !pasteRows(view, d.getData('text/html'), d.getData('text/plain'))) return false
      e.preventDefault()
      return true
    },
  }),
]
