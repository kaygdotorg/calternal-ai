import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { describe, expect, test } from 'vitest'
import { hiddenSyntax } from '../src/syntax/hidden'

const draw = (doc: string) => {
  const view = new EditorView({ state: EditorState.create({ doc, extensions: [new LanguageSupport(markdownLanguage), hiddenSyntax] }), parent: document.body })
  const lines = [...view.dom.querySelectorAll('.cm-line')].map((l) => l.classList.contains('cm-table-pending'))
  view.destroy()
  return lines
}

describe('tables before their code has loaded', () => {
  test('the lines of a table keep the grid height (header, delimiter, rows) and nothing else does', () => {
    expect(draw('# T\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n\nafter\n')).toEqual([false, false, true, true, true, true, false, false, false])
  })
  test('a note with no table, a table inside a quote, and a lone pipe line are left alone', () => {
    expect(draw('# T\n\nplain | text\n\nmore\n').some(Boolean)).toBe(false)
    expect(draw('> | a | b |\n> | --- | --- |\n> | 1 | 2 |\n').some(Boolean)).toBe(false)
    expect(draw('| a | b |\n\nafter\n').some(Boolean)).toBe(false)
  })
})
