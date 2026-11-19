import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { frontmatterEnd } from '../src/syntax/frontmatter'

/** What the old reading (one pattern over the first 64 KB) said; the line-by-line one must agree. */
const PATTERN = /^---\r?\n[\s\S]*?\r?\n---[ \t]*\r?\n/
const byPattern = (doc: string) => PATTERN.exec(doc.slice(0, 64 * 1024))?.[0].length ?? 0

const end = (doc: string) => frontmatterEnd(EditorState.create({ doc }))

describe('frontmatterEnd', () => {
  const cases = [
    '',
    'no frontmatter\n',
    '---\ntitle: A\n---\n# A\n',
    '---\ntitle: A\n---',
    '---\n---\n',
    '---\n---\n---\nbody',
    '---\n\n---\nbody',
    '---\na: 1\n---  \nbody',
    '---\na: 1\n----\nb\n---\nbody',
    '--- \na: 1\n---\nbody',
    '---\na: 1\nnever closed\n',
    `---\n${'k: v\n'.repeat(20_000)}---\nbody`,
    `---\n${'k: v\n'.repeat(13_000)}---\nbody`,
  ]
  for (const doc of cases)
    it(`agrees with the pattern: ${JSON.stringify(doc.slice(0, 30))}`, () => {
      expect(end(doc)).toBe(byPattern(doc))
    })

  it('answers again from memory for the same document', () => {
    const s = EditorState.create({ doc: '---\na: 1\n---\nbody' })
    expect(frontmatterEnd(s)).toBe(13)
    expect(frontmatterEnd(s)).toBe(13)
  })
})
