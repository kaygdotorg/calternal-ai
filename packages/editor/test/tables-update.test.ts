/** The table field keeps its NodeView through its own cell edits and redraws for other document changes. */
import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport } from '@codemirror/language'
import { type AnnotationType, EditorState, StateField, Transaction } from '@codemirror/state'
import type { DecorationSet } from '@codemirror/view'
import { describe, expect, test, vi } from 'vitest'
import * as grid from '../src/tables/grid'
import { tables } from '../src/tables/tables'
import { replaceCellLine } from '../src/tables/model'

interface GridRevision extends grid.GridRevision {
  lines: string[]
  textLength: number
}

interface TableFieldValue {
  set: DecorationSet
}

const NOTE = '| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n| Rice | 3 |'
const MANY_TABLES = Array.from({ length: 12 }, (_, i) => `| Item ${i} | Qty |\n| --- | --- |\n| Oats | 2 |`).join('\n\n')
const field = (tables as unknown as unknown[])[0] as StateField<TableFieldValue>
const editAnnotation =
  (grid as unknown as { tableCellEdit?: AnnotationType<{ revision: GridRevision; version: number; from: number }> }).tableCellEdit

/** Builds the table field with the same Markdown grammar that the editor uses. */
function stateWithTable(doc = NOTE): EditorState {
  return EditorState.create({ doc, extensions: [new LanguageSupport(markdownLanguage), field] })
}

/** Finds the widget object that occupies this table's replacement range. */
function widgetIn(state: EditorState): unknown {
  let found: unknown
  state.field(field).set.between(0, state.doc.length, (_from, _to, decoration) => {
    if (found === undefined && decoration.spec.widget) found = decoration.spec.widget
  })
  return found
}

describe('table state updates', () => {
  /** CodeMirror may ask for this while measuring edits, so the full table's line count is cached at widget creation. */
  test('reuses the table widget height without splitting every table line', () => {
    const state = stateWithTable()
    const widget = widgetIn(state) as { estimatedHeight: number }
    const split = vi.spyOn(String.prototype, 'split')
    const callsBeforeMeasure = split.mock.calls.length

    expect(widget.estimatedHeight).toBe(138)
    expect(widget.estimatedHeight).toBe(138)
    expect(split.mock.calls).toHaveLength(callsBeforeMeasure)
    split.mockRestore()
  })

  /** A grid-tagged cell transaction maps the current widget instead of finding and rebuilding every table. */
  test('a uniquely tagged cell edit keeps the existing table widget', () => {
    expect(editAnnotation, 'the Grid cell-edit annotation must identify its own widget').toBeDefined()
    if (!editAnnotation) return

    const before = stateWithTable()
    const state = before.field(field)
    const widget = widgetIn(before) as { revision?: GridRevision } | undefined
    const revision = widget?.revision
    expect(revision).toBeDefined()
    if (!revision) return
    const line = before.doc.line(3)
    const insert = replaceCellLine(line.text, 0, 'Oats fresh')
    const oldLength = revision.textLength
    revision.lines[2] = insert
    revision.version++

    const tr = before.update({
      changes: { from: line.from, to: line.to, insert },
      annotations: [Transaction.userEvent.of('input.table.cell'), editAnnotation.of({ revision, version: revision.version, from: 0 })],
    })

    expect(tr.annotation(Transaction.userEvent)).toBe('input.table.cell')
    expect(tr.annotation(editAnnotation)).toEqual({ revision, version: 1, from: 0 })
    expect(widgetIn(tr.state)).toBe(widgetIn(before))
    expect(revision.appliedLines[2]).toBe(insert)
    expect(revision.appliedVersion).toBe(1)
    expect(revision.textLength).toBe(oldLength + insert.length - line.length)
  })

  /** A tagged cell edit asks the range tree for its own widget instead of iterating every table in the note. */
  test('looks up one table decoration at the edited cell position', () => {
    expect(editAnnotation, 'the Grid cell-edit annotation must identify its table position').toBeDefined()
    if (!editAnnotation) return

    const before = stateWithTable(MANY_TABLES)
    const value = before.field(field)
    const widget = widgetIn(before) as { revision?: GridRevision } | undefined
    const revision = widget?.revision
    expect(revision).toBeDefined()
    if (!revision) return

    const from = before.doc.line(1).from
    const line = before.doc.line(3)
    const insert = replaceCellLine(line.text, 0, 'Oats fresh')
    revision.lines[2] = insert
    revision.version++
    const between = vi.spyOn(value.set, 'between')
    const tr = before.update({
      changes: { from: line.from, to: line.to, insert },
      annotations: [Transaction.userEvent.of('input.table.cell'), editAnnotation.of({ revision, version: revision.version, from })],
    })
    const after = tr.state

    expect(between).toHaveBeenCalledOnce()
    expect(between).toHaveBeenCalledWith(from, from + 1, expect.any(Function))
    expect(after.doc.line(3).text).toBe(insert)
  })

  /** A cell-like user-event string alone is not trusted to bypass table discovery. */
  test('an untagged cell-like edit redraws the table', () => {
    const before = stateWithTable()
    const line = before.doc.line(3)
    const insert = replaceCellLine(line.text, 0, 'Oats fresh')
    const tr = before.update({
      changes: { from: line.from, to: line.to, insert },
      userEvent: 'input.table.cell',
    })

    expect(widgetIn(tr.state)).not.toBe(widgetIn(before))
  })

  /** A grid identity from another editor cannot skip discovery for this note's table. */
  test('a stale cell revision redraws the table', () => {
    expect(editAnnotation, 'the Grid cell-edit annotation must identify its own widget').toBeDefined()
    if (!editAnnotation) return

    const other = stateWithTable()
    const stale = (widgetIn(other) as { revision?: GridRevision }).revision!
    const before = stateWithTable()
    const line = before.doc.line(3)
    const insert = replaceCellLine(line.text, 0, 'Oats fresh')
    const tr = before.update({
      changes: { from: line.from, to: line.to, insert },
      annotations: [Transaction.userEvent.of('input.table.cell'), editAnnotation.of({ revision: stale, version: 1, from: 0 })],
    })

    expect(widgetIn(tr.state)).not.toBe(widgetIn(before))
  })
})
