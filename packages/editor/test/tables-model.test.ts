/** Parser, serializer and in-place cell edits for the plain Markdown table kept in a note. */
import { describe, expect, test } from 'vitest'
import {
  deleteCol,
  deleteRow,
  emptyTable,
  fromHTML,
  fromTSV,
  insertCol,
  insertRow,
  isNumeric,
  moveCol,
  moveRow,
  parseTable,
  pipeLine,
  replaceCellLine,
  serializeTable,
  setAlign,
  sortBy,
  textFor,
  tidyDelimiterLine,
} from '../src/tables/model'

const GYM = `| Exercise   | Sets | Reps |   kg |
| :--------- | ---: | ---: | ---: |
| Bench      |    4 |    8 | 60.5 |
| Squat \\| x |    5 |    5 |   80 |`

const ESCAPED = `| a \\| b | c   |
| ------ | --- |
| x      | y   |`

/** Tables as people and other tools write them: not padded, ragged, odd spacing. Untouched, they stay as they are. */
const AS_WRITTEN = [
  GYM,
  ESCAPED,
  '| a | b |\n| --- | --- |\n| x | 1 |\n| y | 2.5 |',
  '|a|b|\n|:-:|--:|\n|1|2|',
  'a | b\n--- | ---\n1 | 2',
  '| a | b | c |\n| --- | --- | --- |\n| 1 |\n| 1 | 2 | 3 | 4 |',
  '| a | b |\n| :-- | :-: |\n|  |  |',
  '| only |\n| - |\n| x |',
  '| a | b\\| |\n|---|---|\n| `x\\|y` | z |',
  '| head | head |\n| --- | --- |',
  '| ünï | 日本 |\n| --- | --- |\n| 🙂 | é |',
]

describe('table model', () => {
  test('round trip keeps the alignment row and escapes', () => {
    const t = parseTable(GYM)
    expect(t.align).toEqual(['l', 'r', 'r', 'r'])
    expect(t.rows[2]![0]).toBe('Squat | x')
    expect(serializeTable(t)).toBe(GYM)
  })

  test('an escaped pipe stays one cell and is written back escaped', () => {
    const t = parseTable(ESCAPED)
    expect(t.rows[0]).toEqual(['a | b', 'c'])
    expect(serializeTable(t)).toBe(ESCAPED)
  })

  test('typing changes only the chosen raw cell and keeps every other byte in its row', () => {
    const row = '| 80  | old \\| value |'
    const changed = replaceCellLine(row, 0, '60')
    expect(changed).toBe('| 60 | old \\| value |')
    expect(parseTable(`| a | b |\n| --- | --- |\n${changed}`).rows[1]).toEqual(['60', 'old | value'])
  })

  test('typing escapes a new pipe without changing neighboring cells', () => {
    const row = '| item  | keep |'
    const changed = replaceCellLine(row, 0, 'oats | rice')
    expect(changed).toBe('| oats \\| rice | keep |')
    expect(parseTable(`| a | b |\n| --- | --- |\n${changed}`).rows[1]).toEqual(['oats | rice', 'keep'])
  })

  test('typing into a blank padded cell gives it the usual one-space frame', () => {
    expect(replaceCellLine('|     |     |', 0, 'Name')).toBe('| Name |     |')
    expect(replaceCellLine('|1|2|', 0, '91')).toBe('| 91 |2|')
  })

  test("a user's plain --- row is kept, numbers still count as numbers", () => {
    const t = parseTable('| a | b |\n| --- | --- |\n| x | 1 |\n| y | 2.5 |')
    expect(t.align).toEqual(['', ''])
    expect(isNumeric(t.rows, 1)).toBe(true)
    expect(isNumeric(t.rows, 0)).toBe(false)
    expect(serializeTable(t).split('\n')[1]).toBe('| --- | --- |')
  })

  test('all four alignments are read and written', () => {
    const t = parseTable('| a | b | c | d |\n| --- | :-- | :-: | --: |\n| 1 | 2 | 3 | 4 |')
    expect(t.align).toEqual(['', 'l', 'c', 'r'])
    expect(serializeTable(t).split('\n')[1]).toBe('| --- | :-- | :-: | --: |')
  })

  /** Tidying a legacy delimiter keeps its dashes and alignment markers intact. */
  test('normalizes only delimiter frame spacing', () => {
    expect(tidyDelimiterLine('|:-:|--:|')).toBe('| :-: | --: |')
    expect(tidyDelimiterLine('| :----- | ---: |')).toBe('| :----- | ---: |')
    expect(tidyDelimiterLine('not a delimiter')).toBe('not a delimiter')
  })

  test('ragged rows are made even, the extra cells empty', () => {
    const t = parseTable('| a | b | c |\n| --- | --- | --- |\n| 1 |\n| 1 | 2 | 3 | 4 |')
    expect(t.rows.map((r) => r.length)).toEqual([4, 4, 4])
    expect(t.rows[1]).toEqual(['1', '', '', ''])
    expect(t.align.length).toBe(4)
  })

  test('empty rows are kept (an all-blank row is not a delimiter row)', () => {
    const t = parseTable('| a | b |\n| --- | --- |\n|  |  |')
    expect(t.rows.length).toBe(2)
  })

  test('a table nobody edited keeps the file byte for byte', () => {
    for (const text of AS_WRITTEN) expect(textFor(text, parseTable(text))).toBe(text)
  })

  test('an edited table is written once in the tidy form, and that form is stable', () => {
    for (const text of AS_WRITTEN) {
      const edited = insertRow(parseTable(text), 1)
      const written = textFor(text, edited)
      expect(written).toBe(serializeTable(edited))
      // Reading what we wrote and writing it again changes nothing.
      expect(serializeTable(parseTable(written))).toBe(written)
    }
  })

  test('what we write is a GFM table: header, delimiter, the same number of cells on every line', () => {
    for (const text of AS_WRITTEN) {
      const lines = serializeTable(parseTable(text)).split('\n')
      expect(lines[1]).toMatch(/^\| :?-+:? (\| :?-+:? )*\|$/)
      const cols = parseTable(text).align.length
      for (const l of lines) expect(l.replace(/\\\|/g, '').split('|').length - 2).toBe(cols)
    }
  })

  test('edits', () => {
    const t = parseTable(GYM)
    expect(insertRow(t, 1).rows.length).toBe(4)
    expect(deleteRow(deleteRow(t, 1), 1).rows.length).toBe(2)
    expect(deleteRow(deleteRow(deleteRow(t, 1), 1), 1).rows.length).toBe(2)
    expect(insertCol(t, 0).rows[0]!.length).toBe(5)
    expect(deleteCol(t, 3).align.length).toBe(3)
    expect(moveRow(t, 1, 2).rows[1]![0]).toContain('Squat')
    expect(moveCol(t, 0, 1).rows[0]!.slice(0, 2)).toEqual(['Sets', 'Exercise'])
    expect(setAlign(t, 0, 'c').align[0]).toBe('c')
    expect(sortBy(t, 3, -1).rows[1]![3]).toBe('80')
    expect(sortBy(t, 0, 1).rows[1]![0]).toBe('Bench')
  })

  test('paste and typed lines', () => {
    expect(fromTSV('a\tb\n1\t2\n')).toEqual([
      ['a', 'b'],
      ['1', '2'],
    ])
    expect(fromTSV('no tabs')).toBeNull()
    expect(fromTSV('a\t"b\nc"\n')).toEqual([['a', 'b c']])
    expect(pipeLine('| a | b |')).toEqual(['a', 'b'])
    expect(pipeLine('plain')).toBeNull()
    expect(pipeLine('||')).toBeNull()
    expect(serializeTable(emptyTable()).split('\n').length).toBe(4)
  })

  test('a web page table', () => {
    const rows = fromHTML('<table><tr><th>Item</th><th>Qty</th></tr><tr><td>Oats</td><td>2</td></tr></table>')
    expect(rows).toEqual([
      ['Item', 'Qty'],
      ['Oats', '2'],
    ])
    expect(fromHTML('<p>no table</p>')).toBeNull()
  })
})
