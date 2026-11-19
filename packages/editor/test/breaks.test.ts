import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { EditorSelection, EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { afterEach, describe, expect, it } from 'vitest'
import { breakLines, breaks } from '../src/syntax/breaks'

let views: EditorView[] = []
afterEach(() => {
  views.forEach((v) => v.destroy())
  views = []
})

function view(doc: string, caret = doc.length) {
  const parent = document.createElement('div')
  document.body.append(parent)
  const v = new EditorView({
    state: EditorState.create({ doc, selection: { anchor: caret }, extensions: [markdown({ base: markdownLanguage }), breaks] }),
    parent,
  })
  views.push(v)
  return v
}

/** Types one character the way the keyboard does (through the input handlers). */
function type(v: EditorView, text: string) {
  const { from, to } = v.state.selection.main
  const handled = v.state.facet(EditorView.inputHandler).some((h) => h(v, from, to, text, () => v.state.update({ changes: { from, to, insert: text } })))
  if (!handled) v.dispatch({ changes: { from, to, insert: text }, selection: EditorSelection.cursor(from + text.length) })
}

const lineClasses = (v: EditorView) => [...v.contentDOM.querySelectorAll('.cm-line')].map((l) => l.className)

describe('breaks', () => {
  it('marks the line of a card of one line (cm-lone), with or without blank lines around it', () => {
    const doc = 'One\n\n---\n\nOnly this\n\n---\n\nTwo\nand more\n\n---\n---\nThree\n\n---\nEnd\n'
    const v = view(doc, 0)
    const lone = [...v.contentDOM.querySelectorAll('.cm-lone')].map((l) => l.textContent)
    // "Only this" is alone between two breaks; "Two / and more" is two lines; "Three" sits between a break and a break with no blank lines.
    expect(lone).toEqual(['Only this', 'Three'])
  })

  it('finds each --- line, never the frontmatter fences', () => {
    const doc = '---\ntitle: T\n---\n# T\n\nOne\n\n---\n\nTwo\n\n***\n\nThree\n'
    const v = view(doc, doc.indexOf('Three'))
    expect(breakLines(v)).toEqual([doc.indexOf('---\n\nTwo'), doc.indexOf('***')])
  })
  it('hides the --- unless the caret is on its line', () => {
    const doc = 'One\n\n---\n\nTwo\n'
    const v = view(doc, 0)
    expect(lineClasses(v).filter((c) => c.includes('cm-break'))).toEqual(['cm-line cm-break'])
    expect(v.contentDOM.textContent).not.toContain('---')
    v.dispatch({ selection: { anchor: doc.indexOf('---') } })
    expect(lineClasses(v).filter((c) => c.includes('cm-break'))).toEqual(['cm-line cm-break cm-break-active'])
    expect(v.contentDOM.textContent).toContain('---')
  })
  it('typing --- under a line of text makes a break, not a heading, and moves on', () => {
    const v = view('# T\n\nSome words\n')
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('# T\n\nSome words\n\n---\n')
    expect(v.state.selection.main.head).toBe(v.state.doc.length)
    expect(breakLines(v)).toEqual([v.state.doc.toString().indexOf('---')])
  })
  it('typing --- on a blank line makes the break and puts the caret on a new line', () => {
    const v = view('Some words\n\n')
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('Some words\n\n---\n')
    expect(v.state.selection.main.head).toBe(v.state.doc.length)
  })
  it('reuses the blank line below instead of adding one', () => {
    const v = view('One\n\n\n\nTwo\n', 5)
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('One\n\n---\n\nTwo\n')
    expect(v.state.selection.main.head).toBe(9)
  })
  it('puts a new line between the break and text that follows', () => {
    const v = view('One\n\nTwo\n', 4)
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('One\n\n---\n\nTwo\n')
  })
  it('leaves the first line alone: --- there opens the properties', () => {
    const v = view('')
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('---')
  })
  it('leaves an unclosed --- at the top alone, so a closing --- is plain typing', () => {
    const v = view('---\ntitle: T\n')
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe('---\ntitle: T\n---')
  })
  it('does not touch a note whose properties are closed', () => {
    const doc = '---\ntitle: T\n---\nBody\n\n'
    const v = view(doc)
    for (const c of '---') type(v, c)
    expect(v.state.doc.toString()).toBe(doc + '---\n')
  })
  it('keeps dashes as typed in code blocks and tables', () => {
    const code = view('One\n\n```\ncode\n')
    for (const c of '---') type(code, c)
    expect(code.state.doc.toString()).toBe('One\n\n```\ncode\n---')
    // A table cell's line is never a lone ---, so typing dashes in one is plain typing.
    const table = view('| a | b |\n| - | - |\n| 1 | 2 |')
    for (const c of '---') type(table, c)
    expect(table.state.doc.toString()).toBe('| a | b |\n| - | - |\n| 1 | 2 |---')
  })
  it('only a line that is exactly --- converts', () => {
    const v = view('One\n\n--- text\n', 7)
    type(v, '-')
    expect(v.state.doc.toString()).toBe('One\n\n---- text\n')
    const w = view('One\n\n----\n', 7)
    type(w, '-')
    expect(w.state.doc.toString()).toBe('One\n\n-----\n')
  })
})
