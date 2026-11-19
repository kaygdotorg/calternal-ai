import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { hiddenBlockList, hiddenBlocks, hiddenPeek, setPeek } from '../src/blocks/hidden-blocks'
import { hiddenEdits, hiddenRefs } from '../src/blocks/hidden-refs'

const apply = (text: string, edits: { from: number; to: number; insert: string }[]) =>
  [...edits].sort((a, b) => b.from - a.from).reduce((t, e) => t.slice(0, e.from) + e.insert + t.slice(e.to), text)

describe('hidden block list in the frontmatter', () => {
  it('reads both keys and never counts the note flag', () => {
    expect(hiddenRefs('---\nhidden blocks: ["^k2m8q1w4zre7k3f9", "^ab12cd"]\n---\n')).toEqual(['k2m8q1w4zre7k3f9', 'ab12cd'])
    expect(hiddenRefs('---\nhidden: ["^ab12cd"]\n---\n')).toEqual(['ab12cd'])
    expect(hiddenRefs('---\nhidden: true\n---\n')).toEqual([])
    expect(hiddenRefs('---\nhidden blocks:\n  - ^aa11\n  - "^bb22"\n---\n')).toEqual(['aa11', 'bb22'])
    expect(hiddenRefs('no frontmatter\n')).toEqual([])
  })
  it('makes a frontmatter block when there is none', () => {
    expect(apply('# T\n', hiddenEdits('# T\n', ['ab12']))).toBe('---\nhidden blocks: ["^ab12"]\n---\n# T\n')
  })
  it('adds the line last and leaves other lines alone', () => {
    const t = '---\ntags: [a]\n---\nbody\n'
    expect(apply(t, hiddenEdits(t, ['ab12']))).toBe('---\ntags: [a]\nhidden blocks: ["^ab12"]\n---\nbody\n')
  })
  it('rewrites the list in place and removes it when empty', () => {
    const t = '---\nhidden blocks: ["^ab12"]\ntags: [a]\n---\n'
    expect(apply(t, hiddenEdits(t, ['ab12', 'cd34']))).toBe('---\nhidden blocks: ["^ab12", "^cd34"]\ntags: [a]\n---\n')
    expect(apply(t, hiddenEdits(t, []))).toBe('---\ntags: [a]\n---\n')
  })
  it('moves an old hidden: list and keeps hidden: true', () => {
    const t = '---\nhidden: ["^ab12"]\n---\n'
    expect(apply(t, hiddenEdits(t, ['ab12', 'cd34']))).toBe('---\nhidden blocks: ["^ab12", "^cd34"]\n---\n')
    const flag = '---\nhidden: true\n---\n'
    expect(apply(flag, hiddenEdits(flag, ['ab12']))).toBe('---\nhidden: true\nhidden blocks: ["^ab12"]\n---\n')
  })
})

function state(doc: string) {
  const s = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage }), hiddenBlocks] })
  ensureSyntaxTree(s, s.doc.length, 5000)
  return s
}
const DOC = '---\nhidden blocks: ["^ab12"]\n---\n# T\n\nkeep\n\nsecret ^ab12\n\nend\n'

function covered(s: EditorState): string[] {
  const out: string[] = []
  const v = s.field(hiddenBlocks as never) as { deco: { iter(): { value: unknown; from: number; to: number; next(): void } } }
  for (const it = v.deco.iter(); it.value; it.next()) out.push(s.doc.sliceString(it.from, it.to))
  return out
}

describe('hidden blocks in the reading view', () => {
  it('lists the ids and replaces the block and one blank line, with no placeholder', () => {
    const s = state(DOC)
    expect(hiddenBlockList(s)).toEqual(['ab12'])
    expect(covered(s)).toEqual(['secret ^ab12\n'])
  })
  it('shows the block faded while peeking and puts it away again', () => {
    let s = state(DOC)
    s = s.update({ effects: setPeek.of({ all: true }) }).state
    expect(hiddenPeek(s).all).toBe(true)
    expect(covered(s)).toEqual(['', '']) // the faded line and the unhide widget
    s = s.update({ effects: setPeek.of({ all: false, only: 'ab12' }) }).state
    expect(hiddenPeek(s)).toEqual({ all: false, only: 'ab12' })
    s = s.update({ effects: setPeek.of({ only: null }) }).state
    expect(covered(s)).toEqual(['secret ^ab12\n'])
  })
  it('costs nothing for a note without hidden blocks', () => {
    const s = state('# T\n\nplain\n')
    expect(hiddenBlockList(s)).toEqual([])
    const after = s.update({ changes: { from: 8, insert: 'x' } }).state
    expect(after.field(hiddenBlocks as never)).toBe(s.field(hiddenBlocks as never))
  })
  it('follows the list when the frontmatter changes', () => {
    const s = state(DOC)
    const at = DOC.indexOf('"^ab12"')
    const after = s.update({ changes: { from: at - 1, to: at + 8, insert: '[]' } }).state
    expect(hiddenBlockList(after)).toEqual([])
    expect(covered(after)).toEqual([])
  })
})
