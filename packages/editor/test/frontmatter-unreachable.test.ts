import { deleteCharBackward, deleteCharForward, selectAll } from '@codemirror/commands'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { EditorSelection, EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { ySyncAnnotation } from 'y-codemirror.next'
import { afterEach, describe, expect, it } from 'vitest'
import { frontmatterEnd } from '../src/syntax/frontmatter'
import { hiddenSyntax } from '../src/syntax/hidden'

const PROPS = '---\ntitle: A\ncalternal-width: wide\ncalternal-page-opacity: 60%\n---\n'
const DOC = `${PROPS}# A\n\nBody line.\n`

let views: EditorView[] = []
afterEach(() => {
  views.forEach((v) => v.destroy())
  views = []
})

/** A real editor in the default mode (properties hidden), the caret at `caret` (the body start unless told). */
function view(doc = DOC, caret?: number, extensions: unknown[] = [hiddenSyntax]) {
  const parent = document.createElement('div')
  document.body.append(parent)
  const v = new EditorView({
    state: EditorState.create({
      doc,
      selection: { anchor: caret ?? frontmatterEnd(EditorState.create({ doc })) },
      extensions: [markdown({ base: markdownLanguage }), ...(extensions as [])],
    }),
    parent,
  })
  views.push(v)
  return v
}

const props = (v: EditorView) => v.state.doc.sliceString(0, PROPS.length)

describe('the hidden properties are unreachable', () => {
  it('Backspace x20 at the start of the body changes nothing', () => {
    const doc = `${PROPS}Body line.\n`
    const v = view(doc)
    for (let i = 0; i < 20; i++) deleteCharBackward(v)
    expect(v.state.doc.toString()).toBe(doc)
    expect(v.state.selection.main.head).toBe(PROPS.length)
  })

  it('Backspace at the start of an empty body changes nothing (the closing fence keeps its line break)', () => {
    const v = view(PROPS)
    for (let i = 0; i < 20; i++) deleteCharBackward(v)
    expect(v.state.doc.toString()).toBe(PROPS)
    expect(frontmatterEnd(v.state)).toBe(PROPS.length)
  })

  it('Delete from the end of the note, and from the end of the title, keeps the properties', () => {
    const v = view(DOC, DOC.indexOf('# A') + 3)
    for (let i = 0; i < 30; i++) deleteCharForward(v)
    expect(v.state.doc.toString()).toBe(`${PROPS}# A`)
  })

  it('select all selects the body only, and deleting it leaves the properties', () => {
    const v = view()
    selectAll(v)
    expect(v.state.selection.main.from).toBe(PROPS.length)
    deleteCharBackward(v)
    expect(v.state.doc.toString()).toBe(PROPS)
    expect(v.state.selection.main.head).toBe(PROPS.length)
  })

  it('select all + paste replaces the body and nothing else', () => {
    const v = view()
    selectAll(v)
    v.dispatch({ ...v.state.replaceSelection('# Pasted\n'), userEvent: 'input.paste' })
    expect(v.state.doc.toString()).toBe(`${PROPS}# Pasted\n`)
    expect(v.state.selection.main.head).toBe(v.state.doc.length)
  })

  it('a change that starts in the properties and reaches into the body is refused whole', () => {
    const v = view()
    v.dispatch({ changes: { from: 6, to: PROPS.length + 3, insert: 'X' }, userEvent: 'input' })
    expect(v.state.doc.toString()).toBe(DOC)
  })

  it('changes wholly inside the properties are dropped, whatever their kind', () => {
    const v = view()
    for (const [change, event] of [
      [{ from: 0, insert: 'x' }, 'input.type'],
      [{ from: 0, to: 3, insert: 'xyz' }, 'input'],
      [{ from: 6, insert: 'oops: 1\n' }, 'input.paste'],
      [{ from: PROPS.length - 1, to: PROPS.length }, 'delete.backward'],
      [{ from: 0, to: PROPS.length }, 'delete.selection'],
      [{ from: PROPS.length - 4, to: PROPS.length - 1 }, 'input.type.compose'],
    ] as const)
      v.dispatch({ changes: change, userEvent: event })
    expect(v.state.doc.toString()).toBe(DOC)
  })

  it('typing at the start of the body works', () => {
    const v = view()
    v.dispatch({ changes: { from: PROPS.length, insert: 'hi ' }, userEvent: 'input.type' })
    expect(v.state.doc.toString()).toBe(`${PROPS}hi # A\n\nBody line.\n`)
  })

  it('a caret set inside the properties is moved to the body', () => {
    const v = view()
    v.dispatch({ selection: { anchor: 0 } })
    expect(v.state.selection.main.head).toBeGreaterThanOrEqual(PROPS.length)
    v.dispatch({ selection: EditorSelection.range(2, 40) })
    expect(Math.min(v.state.selection.main.anchor, v.state.selection.main.head)).toBeGreaterThanOrEqual(PROPS.length)
  })

  it('the properties writer (and only it) may change them', () => {
    const v = view()
    const at = PROPS.indexOf('calternal-width')
    v.dispatch({ changes: { from: at, to: at + 'calternal-width: wide'.length, insert: 'calternal-width: narrow' }, userEvent: 'input.properties' })
    expect(v.state.doc.toString()).toContain('calternal-width: narrow')
    expect(frontmatterEnd(v.state)).toBeGreaterThan(0)
    // The caret stayed in the body, shifted by the longer line.
    expect(v.state.selection.main.head).toBe(frontmatterEnd(v.state))
  })

  it('changes from the shared document (another device, the server, undo) are not cut', () => {
    const v = view()
    const at = PROPS.indexOf('calternal-width')
    v.dispatch({ changes: { from: at, insert: 'calternal-id: n1\n' }, annotations: ySyncAnnotation.of({} as never) }) // (only that it is there matters: `isRemote`)
    expect(v.state.doc.toString()).toContain('calternal-id: n1\n')
    expect(v.state.selection.main.head).toBe(frontmatterEnd(v.state))
  })

  it('a note without properties is untouched by the guard', () => {
    const v = view('# Plain\n\nWords.\n', 0)
    v.dispatch({ changes: { from: 0, insert: 'x' } })
    expect(v.state.doc.toString()).toBe('x# Plain\n\nWords.\n')
  })

  it('source mode (no guard) can edit the properties', () => {
    const v = view(DOC, 0, [])
    v.dispatch({ changes: { from: 0, insert: 'x' } })
    expect(v.state.doc.toString()).toBe(`x${DOC}`)
  })
})

describe('the caret of a new state', () => {
  it('starts inside the properties and one selection update moves it to the body', () => {
    const s = EditorState.create({ doc: DOC, extensions: [markdown({ base: markdownLanguage }), hiddenSyntax] })
    expect(s.selection.main.head).toBe(0)
    expect(s.update({ selection: s.selection }).state.selection.main.head).toBeGreaterThanOrEqual(PROPS.length)
  })

  it('stays where it is in source mode (no guard)', () => {
    const s = EditorState.create({ doc: DOC, extensions: [markdown({ base: markdownLanguage })] })
    expect(s.update({ selection: s.selection }).state.selection.main.head).toBe(0)
  })
})
