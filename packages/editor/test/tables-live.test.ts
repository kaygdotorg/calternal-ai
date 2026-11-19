/** Typing in a table cell is an edit of the note itself: the same event, the same document, no copy held back. */
import { history, undo, redo } from '@codemirror/commands'
import { LanguageSupport } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { afterEach, describe, expect, test } from 'vitest'
import { noteMarkdown } from '../src/tables/parser'
import { tables } from '../src/tables/tables'

/** happy-dom has no table row/cell collections; the grid reads them, so give it the standard ones. */
function polyfillTables(): void {
  const defineOnce = (proto: object, name: string, get: (this: HTMLElement) => unknown) => {
    if (!(name in proto)) Object.defineProperty(proto, name, { get })
  }
  defineOnce(HTMLTableSectionElement.prototype, 'rows', function () {
    return Array.from(this.children).filter((e) => e.tagName === 'TR')
  })
  defineOnce(HTMLTableRowElement.prototype, 'cells', function () {
    return Array.from(this.children).filter((e) => e.tagName === 'TD' || e.tagName === 'TH')
  })
  // The grid keeps its rows in several row groups (tbody elements): the table's own list and index span them all.
  defineOnce(HTMLTableElement.prototype, 'rows', function () {
    return Array.from(this.querySelectorAll(':scope > tbody > tr'))
  })
  defineOnce(HTMLTableRowElement.prototype, 'rowIndex', function () {
    const table = this.closest('table')
    return table ? Array.from(table.querySelectorAll(':scope > tbody > tr')).indexOf(this) : -1
  })
}
polyfillTables()

const NOTE = '| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n| Rice | 3 |'
let view: EditorView | null = null
afterEach(() => {
  view?.destroy()
  view = null
})

function open(doc = NOTE): EditorView {
  const parent = document.createElement('div')
  document.body.append(parent)
  view = new EditorView({
    parent,
    state: EditorState.create({ doc, extensions: [new LanguageSupport(noteMarkdown), history(), tables] }),
  })
  return view
}

/** What the browser does when a person types into a cell: the cell's text changes, then `input` fires. */
function type(cell: HTMLElement, text: string): void {
  cell.textContent = text
  cell.dispatchEvent(new Event('input', { bubbles: true }))
}

describe('a key in a cell is a document edit in the same event', () => {
  test('after each key the note already holds the typed text', () => {
    const v = open()
    const cell = v.dom.querySelectorAll<HTMLElement>('.tw tbody tr')[1]!.querySelectorAll<HTMLElement>('.cell')[0]!
    expect(cell.textContent).toBe('Oats')
    let typed = 'Oats'
    for (const ch of ' and rice') {
      typed += ch
      type(cell, typed)
      // No await, no timer, no frame: the document is already changed.
      expect(v.state.doc.toString()).toContain(`| ${typed}`.replace(/ +$/, ''))
      expect(v.state.doc.line(3).text.includes(typed.trim())).toBe(true)
    }
  })

  test('each key is its own transaction on the note, tagged as a cell edit', () => {
    const v = open()
    const seen: string[] = []
    const cell = v.dom.querySelectorAll<HTMLElement>('.tw tbody tr')[1]!.querySelectorAll<HTMLElement>('.cell')[0]!
    const dispatch = v.dispatch.bind(v)
    v.dispatch = ((...specs: Parameters<EditorView['dispatch']>) => {
      const before = v.state.doc
      dispatch(...specs)
      if (v.state.doc !== before) seen.push(v.state.doc.line(3).text)
    }) as EditorView['dispatch']
    for (const t of ['Oatsm', 'Oatsme', 'Oatsmea']) type(cell, t)
    expect(seen).toHaveLength(3)
    expect(seen[2]).toContain('Oatsmea')
  })

  test('undo and redo go through the note and the cell follows the note', () => {
    const v = open()
    const rowCell = () => v.dom.querySelectorAll<HTMLElement>('.tw tbody tr')[1]!.querySelectorAll<HTMLElement>('.cell')[0]!
    type(rowCell(), 'Oats!')
    expect(v.state.doc.line(3).text).toContain('Oats!')
    expect(undo(v)).toBe(true)
    expect(v.state.doc.toString()).toBe(NOTE)
    expect(rowCell().textContent).toBe('Oats')
    expect(redo(v)).toBe(true)
    expect(v.state.doc.line(3).text).toContain('Oats!')
    expect(rowCell().textContent).toBe('Oats!')
  })

  test('other rows of the note are byte-for-byte what they were', () => {
    const v = open()
    const cell = v.dom.querySelectorAll<HTMLElement>('.tw tbody tr')[2]!.querySelectorAll<HTMLElement>('.cell')[1]!
    type(cell, '30')
    const lines = v.state.doc.toString().split('\n')
    expect(lines.slice(0, 3)).toEqual(NOTE.split('\n').slice(0, 3))
    expect(lines[3]).toContain('30')
  })

  test('a long table is drawn in row groups of 16, and a key in a later group edits its own row', () => {
    const body = Array.from({ length: 40 }, (_, i) => `| r${i + 1} | ${i + 1} |`)
    const v = open(['| Item | Qty |', '| --- | --- |', ...body].join('\n'))
    // 41 rows (header + 40): groups of 16, 16 and 9, in order.
    const groups = Array.from(v.dom.querySelectorAll('.tw table > tbody'), (g) => g.children.length)
    expect(groups).toEqual([16, 16, 9])
    // Row 20 of the table (header is row 0) is in the second group; its line is the note's line 22 (after the header
    // and the delimiter line).
    const cell = v.dom.querySelectorAll<HTMLElement>('.tw tbody tr')[20]!.querySelectorAll<HTMLElement>('.cell')[0]!
    expect(cell.textContent).toBe('r20')
    type(cell, 'r20!')
    expect(v.state.doc.line(22).text).toBe('| r20! | 20 |')
    expect(v.state.doc.line(23).text).toBe('| r21 | 21 |')
    expect(v.state.doc.line(21).text).toBe('| r19 | 19 |')
  })
})
