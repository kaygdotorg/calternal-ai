import { markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree, LanguageSupport } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { continueList } from '../src/lists/enter'

/** Presses Return where `|` is; returns the text with `|` at the caret, or null if not handled. */
function enter(doc: string): string | null {
  const at = doc.indexOf('|')
  let s = EditorState.create({
    doc: doc.replace('|', ''),
    selection: { anchor: at },
    extensions: new LanguageSupport(markdownLanguage),
  })
  // Finish the parse, then hand the full tree to the state (a busy machine can stop the first one early).
  ensureSyntaxTree(s, s.doc.length, 1e9)
  s = s.update({}).state
  let handled = false
  continueList({
    state: s,
    dispatch: (tr) => {
      s = tr.state
      handled = true
    },
  })
  if (!handled) return null
  const head = s.selection.main.head
  return s.doc.sliceString(0, head) + '|' + s.doc.sliceString(head)
}

describe('Return in a list (#225)', () => {
  it('adds one bullet on the next line in a tight list', () => {
    expect(enter('- a\n- b|')).toBe('- a\n- b\n- |')
  })

  it('adds no blank line once the list has one (kayg’s note)', () => {
    expect(enter('- [log](<x.md>)\n\n\n\n- one\n- two|')).toBe('- [log](<x.md>)\n\n\n\n- one\n- two\n- |')
    expect(enter('- a\n\n- b|')).toBe('- a\n\n- b\n- |')
  })

  it('ends the list on an empty item, even after a blank line', () => {
    expect(enter('- a\n- b\n- |')).toBe('- a\n- b\n|')
    expect(enter('- a\n\n- |')).toBe('- a\n\n|')
    // The second item of a two-item list: CodeMirror would push it down a line instead.
    expect(enter('- a\n- |')).toBe('- a\n|')
  })

  it('steps out one level from an empty nested item', () => {
    expect(enter('- a\n  - b\n  - |')).toBe('- a\n  - b\n- |')
  })

  it('keeps numbering and tasks', () => {
    expect(enter('1. a\n\n2. b|')).toBe('1. a\n\n2. b\n3. |')
    expect(enter('- [x] a|')).toBe('- [x] a\n- [ ] |')
  })

  it('splits a log line at the caret into two items', () => {
    expect(enter('- 09:30 Meet |Gabriel')).toBe('- 09:30 Meet\n- |Gabriel')
  })

  it('leaves headings and plain text to the plain Return', () => {
    expect(enter('# Title|')).toBeNull()
    expect(enter('Some words|')).toBeNull()
  })
})
