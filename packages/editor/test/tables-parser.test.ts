/** The note's Markdown language finds the same tables as the stock one, but never parses the cells inline. */
import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport, ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { afterEach, describe, expect, test, vi } from 'vitest'
import { countCells, noteMarkdown } from '../src/tables/parser'

const tablesOf = (parser: { parse(s: string): import('@lezer/common').Tree }, doc: string) => {
  const out: [string, number, number][] = []
  parser.parse(doc).iterate({ enter: (n) => void (n.name === 'Table' && out.push([n.name, n.from, n.to])) })
  return out
}
const names = (parser: { parse(s: string): import('@lezer/common').Tree }, doc: string) => {
  const out = new Set<string>()
  parser.parse(doc).iterate({ enter: (n) => void out.add(n.name) })
  return out
}
/** The inline half of the Markdown parser, which the Lezer types leave out. */
type InlineParser = { parseInline(text: string, offset: number): unknown }

const grid = (rows: number) =>
  ['| Name | Note |', '| --- | ---: |', ...Array.from({ length: rows }, (_, i) => `| **Row ${i}** | see [x](<a.md>) \`c\` |`)].join('\n')

afterEach(() => vi.restoreAllMocks())

describe('table ranges match the stock parser', () => {
  const docs: Record<string, string> = {
    alone: grid(3),
    'with text around': `intro\n\n${grid(4)}\n\nafter`,
    'two tables, a blank line between': `${grid(2)}\n\n${grid(2)}`,
    'two tables, none between (the second header ends the first)': `${grid(2)}\n| A | B |\n| - | - |\n| 1 | 2 |`,
    'header without a delimiter row': '| a | b |\nnot a delimiter\n| c | d |',
    'delimiter with the wrong cell count': '| a | b |\n| --- |\n| c | d |',
    'escaped pipes': '| a \\| b | c |\n| --- | --- |\n| 1 \\| 2 | 3 |',
    'no outer pipes': 'a | b\n--- | ---\n1 | 2',
    'in a list': '- item\n\n  | a | b |\n  | - | - |\n  | 1 | 2 |',
    'in a quote': '> | a | b |\n> | - | - |\n> | 1 | 2 |',
    'after a paragraph with no blank line': 'text\n| a | b |\n| - | - |\n| 1 | 2 |',
    'in a code fence': '```\n| a | b |\n| - | - |\n```',
    'empty rows': '| a | b |\n| - | - |\n|   |   |\n|   |   |',
  }
  for (const [name, doc] of Object.entries(docs))
    test(name, () => expect(tablesOf(noteMarkdown.parser, doc)).toEqual(tablesOf(markdownLanguage.parser, doc)))
})

describe('no inline work inside a table', () => {
  test('cells carry no inline nodes, the stock parser gives them', () => {
    const doc = grid(3)
    for (const inline of ['StrongEmphasis', 'Link', 'InlineCode']) {
      expect(names(markdownLanguage.parser, doc).has(inline)).toBe(true)
      expect(names(noteMarkdown.parser, doc).has(inline)).toBe(false)
    }
    expect(names(noteMarkdown.parser, doc).has('Table')).toBe(true)
  })

  test('text outside a table is still parsed inline', () => {
    const doc = `**bold** and [a](<b.md>)\n\n${grid(2)}`
    expect(names(noteMarkdown.parser, doc).has('StrongEmphasis')).toBe(true)
    expect(names(noteMarkdown.parser, doc).has('Link')).toBe(true)
  })

  /** The count of inline parses after one cell edit does not depend on how big the table is. */
  test('a cell edit costs the same inline work for 10 rows and 400', () => {
    const calls = (rows: number, parser: typeof noteMarkdown.parser) => {
      // The runtime parser has `parseInline`, but the Lezer `Parser` type does not declare it.
      const spy = vi.spyOn(parser as unknown as InlineParser, 'parseInline')
      const lang = noteMarkdown.parser === parser ? new LanguageSupport(noteMarkdown) : new LanguageSupport(markdownLanguage)
      const doc = grid(rows)
      let state = EditorState.create({ doc, extensions: [lang] })
      ensureSyntaxTree(state, state.doc.length, 1e9)
      spy.mockClear()
      const at = doc.indexOf('Row 5')
      state = state.update({ changes: { from: at, to: at + 5, insert: 'Row five' } }).state
      ensureSyntaxTree(state, state.doc.length, 1e9)
      const n = spy.mock.calls.length
      spy.mockRestore()
      return n
    }
    expect(calls(10, noteMarkdown.parser)).toBe(0)
    expect(calls(400, noteMarkdown.parser)).toBe(0)
    // The control: the stock parser's inline work does grow with the table, so this check can fail.
    expect(calls(400, markdownLanguage.parser)).toBeGreaterThan(calls(10, markdownLanguage.parser))
  })
})

describe('countCells', () => {
  test('counts cells like the stock parser, escaped pipes aside', () => {
    expect(countCells('| a | b |')).toBe(2)
    expect(countCells('a | b')).toBe(2)
    expect(countCells('| a \\| b | c |')).toBe(2)
    expect(countCells('| --- |')).toBe(1)
    expect(countCells('|   |   |')).toBe(2)
  })
})
