import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { blockAt } from '../src/blocks/blocks'
import { frontmatterEnd } from '../src/syntax/frontmatter'
import { buildHidden, prefixEnd } from '../src/syntax/hidden'

function state(doc: string) {
  const s = EditorState.create({ doc, extensions: markdown({ base: markdownLanguage }) })
  ensureSyntaxTree(s, s.doc.length, 5000)
  return s
}

/** The text each atomic (hidden or replaced) range covers. */
function hiddenText(doc: string): string[] {
  const s = state(doc)
  const out: string[] = []
  const iter = buildHidden(s).atomic.iter()
  for (; iter.value; iter.next()) out.push(s.doc.sliceString(iter.from, iter.to))
  return out
}

describe('hidden syntax', () => {
  it('hides heading marks with their space', () => {
    expect(hiddenText('## Plan\n')).toEqual(['## '])
  })
  it('hides emphasis and strike marks, keeps the words', () => {
    expect(hiddenText('a **b** _c_ ~~d~~\n')).toEqual(['**', '**', '_', '_', '~~', '~~'])
  })
  it('hides link syntax and keeps the label', () => {
    const s = state('see [Trip](<Trip ideas.md>) now\n')
    const h = buildHidden(s)
    expect(hiddenText('see [Trip](<Trip ideas.md>) now\n')).toEqual(['[', '](<Trip ideas.md>)'])
    expect(h.links).toEqual([
      { from: 4, to: 27, textFrom: 5, textTo: 9, href: 'Trip ideas.md', kind: 'note' },
    ])
  })
  it('tells web links from note links', () => {
    const h = buildHidden(state('[a](https://x.org) [b](b.md) <https://y.org>\n'))
    expect(h.links.map((l) => l.kind)).toEqual(['web', 'note', 'web'])
  })
  it('draws a checkbox for every task state', () => {
    for (const box of ['[ ]', '[x]', '[/]', '[-]']) {
      expect(hiddenText(`- ${box} buy milk\n`)).toEqual([`- ${box} `])
    }
  })
  it('draws a bullet for plain list items, keeps numbers', () => {
    expect(hiddenText('- one\n- two\n')).toEqual(['- ', '- '])
    expect(hiddenText('1. one\n')).toEqual([])
  })
  it('hides quote marks', () => {
    expect(hiddenText('> quoted\n')).toEqual(['> '])
  })
  it('does not touch markdown inside code blocks', () => {
    const hidden = hiddenText('```\n# not a heading **x**\n```\n')
    expect(hidden).toEqual(['```', '```'])
  })
  it('never decorates inside frontmatter', () => {
    const doc = '---\ntitle: A\ndate: 2026-10-13\n---\n# A\n'
    expect(frontmatterEnd(state(doc))).toBe(doc.indexOf('# A'))
    expect(hiddenText(doc)).toEqual(['# '])
  })
})

describe('prefixEnd', () => {
  it('skips hidden leading syntax', () => {
    const s = state('## Plan\n- [ ] task\n> q\nplain\n')
    expect(prefixEnd(s, 0)).toBe(3)
    expect(prefixEnd(s, s.doc.line(2).from)).toBe(s.doc.line(2).from + 6)
    expect(prefixEnd(s, s.doc.line(3).from)).toBe(s.doc.line(3).from + 2)
    expect(prefixEnd(s, s.doc.line(4).from)).toBe(s.doc.line(4).from)
  })
  it('leaves code lines alone', () => {
    const s = state('```\n# x\n```\n')
    expect(prefixEnd(s, s.doc.line(2).from)).toBe(s.doc.line(2).from)
  })
})

describe('blocks', () => {
  const doc = '---\ntitle: T\n---\n# T\n\nPara one\ncontinues\n\n- a\n  - b\n\n## Sub\n'
  const s = state(doc)
  const at = (text: string) => blockAt(s, doc.indexOf(text))
  it('has no block on the title, blank lines or frontmatter', () => {
    expect(at('# T')).toBeNull()
    expect(at('title: T')).toBeNull()
    expect(blockAt(s, doc.indexOf('\n\nPara') + 1)).toBeNull()
  })
  it('treats a paragraph as one block', () => {
    const b = at('continues')!
    expect(s.doc.sliceString(b.from, b.to)).toBe('Para one\ncontinues')
  })
  it('makes each list item its own block, without its children', () => {
    const a = at('- a')!
    expect(s.doc.sliceString(a.from, a.to)).toBe('- a')
    const b = at('- b')!
    expect(s.doc.sliceString(b.from, b.to)).toBe('- b')
  })
  it('gives smaller headings a block', () => {
    expect(at('## Sub')?.kind).toBe('ATXHeading2')
  })
})
