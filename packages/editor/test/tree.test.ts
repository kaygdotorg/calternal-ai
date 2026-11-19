import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { describe, expect, it } from 'vitest'
import { treeShape } from '../src/lists/shape'
import { classifyLine, treeGrow, treeLines, treeRegion } from '../src/lists/tree-lines'

function state(doc: string) {
  const s = EditorState.create({ doc, extensions: markdown({ base: markdownLanguage }) })
  ensureSyntaxTree(s, s.doc.length, 5000)
  return s
}

/** A compact picture of each line: depth, branch and how many guides pass through. */
function picture(doc: string): string[] {
  const s = state(doc.replace(/\n$/, ''))
  const r = treeRegion(s, 1, s.doc.lines)
  return treeShape(r.lines.map((c) => c.line)).map((t, i) => {
    const text = s.doc.line(r.first + i).text.trim()
    if (!t) return `· ${text}`
    const rails = (t.style.match(/linear-gradient/g) ?? []).length
    const b = t.branch === 'tee' ? '├' : t.branch === 'end' ? '└' : ' '
    return `${t.depth}${b}${rails} ${text}`
  })
}

describe('tree shape', () => {
  it('branches each child off its parent: ├ for all but the last, └ for the last', () => {
    expect(picture('- a\n  - b\n  - c\n- d\n')).toEqual(['0 1 - a', '1├2 - b', '1└2 - c', '0 1 - d'])
  })

  it('keeps an ancestor guide running past deeper items while that ancestor has more children', () => {
    // b has a sibling (e) below, so the guide from a passes through c and d.
    expect(picture('- a\n  - b\n    - c\n    - d\n  - e\n')).toEqual([
      '0 1 - a',
      '1├2 - b', // a's guide carries on below the elbow, plus b's own stem
      '2├3 - c', // a's guide, b's guide, and c's own stem to d
      '2└3 - d', // a's and b's guides, and the one arriving from c
      '1└1 - e', // the guide arriving from b
    ])
  })

  it('treats tasks, numbers and bullets alike, four levels deep', () => {
    const doc = ['1. Plan', '   - [ ] book', '     - call', '       1. ask', '- [x] done'].join('\n')
    expect(picture(doc)).toEqual(['0 1 1. Plan', '1└1 - [ ] book', '2└1 - call', '3└0 1. ask', '0 0 - [x] done'])
  })

  it('carries guides through blank lines and text indented under an item', () => {
    expect(picture('- a\n\n  more about a\n  - b\n- c\n')).toEqual(['0 1 - a', '0 1 ', '0 1 more about a', '1└1 - b', '0 1 - c'])
  })

  it('a paragraph ends the list; the next list starts fresh', () => {
    expect(picture('- a\n  - b\nText\n- c\n')).toEqual(['0 1 - a', '1└0 - b', '· Text', '0 0 - c'])
  })

  it('reuses one shape object for lines that look the same', () => {
    const shapes = treeShape(Array.from({ length: 10 }, () => ({ kind: 'item' as const, indent: 2, marker: 'bullet' as const })))
    expect(new Set(shapes).size).toBeLessThanOrEqual(3)
  })

  it('joins siblings at the top level and ends at the last one, with no tail', () => {
    // a's stem runs to b, b's to c; the last item only has the guide arriving from above.
    expect(picture('- a\n- b\n- c\n')).toEqual(['0 1 - a', '0 2 - b', '0 1 - c'])
    expect(picture('- only\n')).toEqual(['0 0 - only'])
  })

  it("draws the guide on to the next sibling past a parent's children", () => {
    // a has children and a sibling: its guide runs through b and c to the next top-level item.
    expect(picture('- a\n  - b\n  - c\n- d\n- e\n')).toEqual(['0 1 - a', '1├2 - b', '1└2 - c', '0 2 - d', '0 1 - e'])
  })

  it('joins numbered items and tasks the same way, and a paragraph cuts the guide', () => {
    expect(picture('1. a\n2. b\n- [ ] c\n- [ ] d\nText\n- e\n- f\n')).toEqual(['0 1 1. a', '0 1 2. b', '0 1 - [ ] c', '0 1 - [ ] d', '· Text', '0 1 - e', '0 1 - f'])
  })
})

describe('tree shape, kinds of block', () => {
  it('joins only siblings of the same kind: a change of kind ends the guide', () => {
    // Nothing runs from b to the to-do, nor from the to-do to the number; each run stands alone.
    expect(picture('- a\n- b\n- [ ] c\n- [ ] d\n1. e\n')).toEqual(['0 1 - a', '0 1 - b', '0 1 - [ ] c', '0 1 - [ ] d', '0 0 1. e'])
  })
})

describe('classifying lines', () => {
  it('knows items, tasks and numbers, and not rules or emphasis', () => {
    expect(classifyLine('  - [ ] x').line).toEqual({ kind: 'item', indent: 2, marker: 'task' })
    expect(classifyLine('\t1) x').line).toEqual({ kind: 'item', indent: 4, marker: 'number' })
    expect(classifyLine('-').line).toEqual({ kind: 'item', indent: 0, marker: 'bullet' })
    expect(classifyLine('---').line.kind).toBe('break')
    expect(classifyLine('*emphasis*').line.kind).toBe('break')
    expect(classifyLine('1.5 kg').line.kind).toBe('break')
    expect(classifyLine('   ').line.kind).toBe('more')
  })

  it('never draws inside a code block at the margin', () => {
    const s = state('```\n- not\n  - a list\n```\n')
    const r = treeRegion(s, 2, 3)
    expect(treeShape(r.lines.map((c) => c.line)).filter(Boolean)).toEqual([])
  })

  it('skips the properties at the top', () => {
    const s = state('---\ntitle: x\n---\n- a\n  - b\n')
    expect(treeRegion(s, 1, s.doc.lines).first).toBe(4)
  })
})

describe('tree lines in the editor', () => {
  function view(doc: string) {
    const parent = document.createElement('div')
    document.body.append(parent)
    return new EditorView({ parent, state: EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage }), treeLines] }) })
  }

  it('never changes the text, and draws the spaces as a spacer of whole steps', () => {
    const doc = '- a\n    - b\n        - c\n'
    const v = view(doc)
    expect(v.state.doc.toString()).toBe(doc)
    const spacers = [...v.contentDOM.querySelectorAll<HTMLElement>('.cm-tree-indent')].map((e) => e.style.getPropertyValue('--tree-depth'))
    expect(spacers).toEqual(['1', '2'])
    const lines = [...v.contentDOM.querySelectorAll<HTMLElement>('.cm-line')]
    expect(lines.map((l) => l.classList.contains('tree-end'))).toEqual([false, true, true, false])
    v.destroy()
  })

  it('grows a branch only for a pointer change, never for typing', () => {
    const v = view('- a\n- b\n')
    v.dispatch({ changes: { from: 4, insert: '  ' }, userEvent: 'input' })
    expect(v.contentDOM.querySelector('.tree-grow')).toBeNull()
    v.dispatch({ changes: { from: 4, to: 6 }, userEvent: 'input' })
    v.dispatch({ changes: { from: 4, insert: '  ' }, annotations: treeGrow.of(true) })
    expect(v.contentDOM.querySelector('.tree-grow')).not.toBeNull()
    v.destroy()
  })
})
