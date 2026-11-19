/** A cell edit patches its Markdown rows and maps the existing table decoration. */
import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport } from '@codemirror/language'
import { type AnnotationType, EditorState, StateField, Transaction } from '@codemirror/state'
import type { DecorationSet } from '@codemirror/view'
import { describe, expect, test, vi } from 'vitest'
import * as grid from '../src/tables/grid'
import * as model from '../src/tables/model'
import { tableRanges, tables } from '../src/tables/tables'

interface GridRevision {
  version: number
  appliedVersion: number
  lines: string[]
  appliedLines: string[]
  textLength: number
}

interface TablesValue {
  set: DecorationSet
  count: number
  docLength: number
  complete: boolean
}

const NOTE = '| Item | Qty |\n| --- | --- |\n| Oats | 2 |\n| Rice    | 3 |'
const field = (tables as unknown as unknown[])[0] as StateField<TablesValue>
const editAnnotation = (grid as unknown as { tableCellEdit?: AnnotationType<{ revision: GridRevision; version: number; from: number }> }).tableCellEdit
const replaceCellLine = (model as unknown as { replaceCellLine?: (line: string, column: number, value: string) => string }).replaceCellLine

/** Builds the table state without drawing a DOM widget, so the field's update cost is isolated. */
function stateWithTable(doc = NOTE): EditorState {
  return EditorState.create({ doc, extensions: [new LanguageSupport(markdownLanguage), tables] })
}

/** Finds the widget object that owns the table's replacement range. */
function widgetIn(state: EditorState): unknown {
  let widget: unknown
  state.field(field).set.between(0, state.doc.length, (_from, _to, decoration) => {
    if (decoration.spec.widget) widget = decoration.spec.widget
  })
  return widget
}

describe('table cell updates', () => {
  /** The row patch retains bytes from neighboring cells and rows. */
  test('patches only the changed raw cell row', () => {
    expect(replaceCellLine).toBeDefined()
    if (!replaceCellLine) return

    expect(replaceCellLine('| Oats | 2 |', 0, 'Oats fresh')).toBe('| Oats fresh | 2 |')
    expect(replaceCellLine('| Rice    | 3 |', 0, 'Rice fresh')).toBe('| Rice fresh | 3 |')
  })

  /** A tagged cell edit should map the current table decoration instead of finding and drawing tables again. */
  test('keeps the current widget while mapping the edited table range', () => {
    expect(editAnnotation, 'the Grid cell-edit annotation must identify its own revision').toBeDefined()
    if (!editAnnotation) return

    const before = stateWithTable()
    const widget = widgetIn(before) as { revision?: GridRevision } | undefined
    expect(widget).toBeDefined()
    expect(widget?.revision).toBeDefined()
    if (!widget?.revision) return

    const revision = widget.revision
    const oldRange = tableRanges(before)[0]!
    const line = before.doc.line(3)
    const insert = replaceCellLine ? replaceCellLine(line.text, 0, 'Oats fresh') : '| Oats fresh | 2 |'
    const oldLength = revision.textLength
    revision.lines[2] = insert
    revision.version++

    const tr = before.update({
      changes: { from: line.from, to: line.to, insert },
      annotations: [
        Transaction.userEvent.of('input.table.cell'),
        editAnnotation.of({ revision, version: revision.version, from: oldRange.from }),
      ],
    })

    expect(widgetIn(tr.state)).toBe(widget)
    expect(tableRanges(tr.state)[0]!.to).toBe(oldRange.to + insert.length - line.length)
    expect(tr.annotation(Transaction.userEvent)).toBe('input.table.cell')
    expect(revision.appliedLines[2]).toBe(insert)
    expect(revision.appliedVersion).toBe(1)
    expect(revision.textLength).toBe(oldLength + insert.length - line.length)
  })

  /** A first cell edit may tidy the delimiter row without redrawing the grid or scanning its table. */
  test('maps a cell edit and its one-time delimiter tidy in place', () => {
    const before = stateWithTable('|a|b|\n|:-:|--:|\n|1|2|\n|3|4|')
    const widget = widgetIn(before) as { revision?: GridRevision } | undefined
    expect(widget?.revision).toBeDefined()
    if (!widget?.revision) return

    const revision = widget.revision
    const oldRange = tableRanges(before)[0]!
    const first = before.doc.lineAt(oldRange.from)
    const delimiter = before.doc.line(first.number + 1)
    const row = before.doc.line(first.number + 2)
    const tidyDelimiter = '| :-: | --: |'
    const changedCell = replaceCellLine!(row.text, 0, '11')
    revision.lines[1] = tidyDelimiter
    revision.lines[2] = changedCell
    revision.version++

    const tr = before.update({
      changes: [
        { from: delimiter.from, to: delimiter.to, insert: tidyDelimiter },
        { from: row.from, to: row.to, insert: changedCell },
      ],
      annotations: [
        Transaction.userEvent.of('input.table.cell'),
        editAnnotation!.of({ revision, version: revision.version, from: oldRange.from }),
      ],
    })

    expect(widgetIn(tr.state)).toBe(widget)
    expect(tableRanges(tr.state)[0]!.to).toBe(
      oldRange.to + tidyDelimiter.length - delimiter.text.length + changedCell.length - row.text.length,
    )
    expect(revision.appliedLines.slice(0, 3)).toEqual([revision.lines[0], tidyDelimiter, changedCell])
    expect(revision.appliedVersion).toBe(1)
  })

  /** A table cell update must query its own decoration, independent of other tables in the note. */
  test('queries only the changed table in a note with many tables', () => {
    const tableCount = 32
    const note = Array.from(
      { length: tableCount },
      (_, i) => `| Name | Qty |\n| --- | --- |\n| Item ${i} | 1 |\n| Spare ${i} | 2 |`,
    ).join('\n\n')
    const before = stateWithTable(note)
    const ranges = tableRanges(before)
    expect(ranges).toHaveLength(tableCount)

    const oldRange = ranges.at(-1)!
    const widget = widgetIn(before) as { revision?: GridRevision } | undefined
    expect(widget?.revision).toBeDefined()
    if (!widget?.revision) return

    const revision = widget.revision
    const header = before.doc.lineAt(oldRange.from)
    const line = before.doc.line(header.number + 2)
    const insert = replaceCellLine!(line.text, 0, 'Final item')
    const oldLength = revision.textLength
    revision.lines[2] = insert
    revision.version++

    const query = vi.spyOn(before.field(field).set, 'between')
    try {
      const tr = before.update({
        changes: { from: line.from, to: line.to, insert },
        annotations: [
          Transaction.userEvent.of('input.table.cell'),
          editAnnotation!.of({ revision, version: revision.version, from: oldRange.from }),
        ],
      })
      const after = tr.state

      expect(query).toHaveBeenCalledTimes(1)
      expect(query.mock.calls[0]?.slice(0, 2)).toEqual([oldRange.from, oldRange.from + 1])
      expect(widgetIn(after)).toBe(widget)
      expect(tableRanges(after).at(-1)?.to).toBe(oldRange.to + insert.length - line.length)
      expect(revision.appliedLines[2]).toBe(insert)
      expect(revision.appliedVersion).toBe(1)
      expect(revision.textLength).toBe(oldLength + insert.length - line.length)
    } finally {
      query.mockRestore()
    }
  })
})
