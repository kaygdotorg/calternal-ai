import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { describe, expect, test } from 'vitest'
import { paintInline, hasInline } from '../src/tables/inline'
import { readWidths, widthsText, writeWidths } from '../src/tables/widths'
import { afterInsert, afterMove, afterRemove } from '../src/tables/motion'

const html = (md: string) => {
  const el = document.createElement('div')
  paintInline(el, md)
  return el.innerHTML
}

describe('inline markdown in a cell', () => {
  test('draws bold, italic, strike, mark, code and links', () => {
    expect(html('**a** and *b*')).toBe('<strong>a</strong> and <em>b</em>')
    expect(html('~~x~~ ==y== `z`')).toBe('<s>x</s> <mark>y</mark> <code>z</code>')
    expect(html('[Plan](<Notes/Plan.md>)')).toBe('<a class="tw-link" data-href="Notes/Plan.md">Plan</a>')
    expect(html('see [x](https://a.b)')).toContain('data-href="https://a.b"')
  })
  test('plain words and stray marks are left alone, and nothing is ever HTML', () => {
    expect(hasInline('plain words')).toBe(false)
    expect(hasInline('snake_case_name 2*3')).toBe(false)
    expect(html('<b>no</b> **yes**')).toBe('&lt;b&gt;no&lt;/b&gt; <strong>yes</strong>')
  })
})

describe('table-widths in the frontmatter', () => {
  const state = (doc: string) => EditorState.create({ doc })
  const view = (doc: string) => new EditorView({ state: state(doc), parent: document.body })

  test('reads lists by table, nulls for columns that size themselves', () => {
    expect(readWidths(state('---\ntitle: x\ntable-widths: [[14, null, 4], null, [3]]\n---\nbody\n'))).toEqual([[14, null, 4], null, [3]])
    expect(readWidths(state('no frontmatter'))).toEqual([])
    expect(readWidths(state('---\ntable-widths: nonsense\n---\nbody\n'))).toEqual([])
  })
  test('the value is the plain JSON list the mockup and ADR 0068 show', () => {
    expect(widthsText([[14, 4, 4, 5]])).toBe('[[14, 4, 4, 5]]')
    expect(widthsText([null, [1, null]])).toBe('[null, [1, null]]')
    expect(widthsText([[null, null]])).toBeNull()
  })
  test('writes into existing frontmatter, makes it when there is none, and removes it when it was the only property', () => {
    let v = view('---\ntitle: x\n---\nbody\n')
    writeWidths(v, 0, 1, [14, null])
    expect(v.state.doc.toString()).toBe('---\ntitle: x\ntable-widths: [[14, null]]\n---\nbody\n')
    writeWidths(v, 0, 1, [20, 5])
    expect(v.state.doc.toString()).toBe('---\ntitle: x\ntable-widths: [[20, 5]]\n---\nbody\n')
    writeWidths(v, 0, 1, null)
    expect(v.state.doc.toString()).toBe('---\ntitle: x\n---\nbody\n')
    v = view('body\n')
    writeWidths(v, 1, 2, [7])
    expect(v.state.doc.toString()).toBe('---\ntable-widths: [null, [7]]\n---\nbody\n')
    writeWidths(v, 1, 2, null)
    expect(v.state.doc.toString()).toBe('body\n')
  })
})

describe('where rows and columns come from after a change', () => {
  test('insert, remove, move', () => {
    expect([0, 1, 2, 3].map(afterInsert(2))).toEqual([0, 1, -1, 2])
    expect([0, 1, 2].map(afterRemove(1))).toEqual([0, 2, 3])
    expect([0, 1, 2, 3].map(afterMove(1, 3))).toEqual([0, 2, 3, 1])
    expect([0, 1, 2, 3].map(afterMove(3, 1))).toEqual([0, 3, 1, 2])
  })
})
