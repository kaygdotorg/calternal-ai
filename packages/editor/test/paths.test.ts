import { describe, expect, it } from 'vitest'
import { markdownLink, relativePath } from '../src/transforms/paths'
import { minimalEdit, replay } from '../src/offline/replay'

describe('relativePath', () => {
  it('links siblings by name, never with ./', () => {
    expect(relativePath('Notes/A.md', 'Notes/Trip ideas.md')).toBe('Trip ideas.md')
  })
  it('walks up with ..', () => {
    expect(relativePath('Notes/Old/A.md', 'Notes/B.md')).toBe('../B.md')
    expect(relativePath('Notes/A.md', 'Documents/x/B.md')).toBe('../Documents/x/B.md')
  })
  it('walks down into folders', () => {
    expect(relativePath('Notes/A.md', 'Notes/Old/B.md')).toBe('Old/B.md')
  })
})

describe('markdownLink', () => {
  it('uses angle brackets so spaces are fine', () => {
    expect(markdownLink('Trip ideas', 'Trip ideas.md')).toBe('[Trip ideas](<Trip ideas.md>)')
  })
  it('escapes brackets in titles', () => {
    expect(markdownLink('a [b]', 'x.md')).toBe('[a \\[b\\]](<x.md>)')
  })
})

describe('replay', () => {
  it('applies only what changed since the base', () => {
    const base = 'one\ntwo\nthree\n'
    const local = 'one\ntwo and more\nthree\n'
    const server = 'zero\none\ntwo\nthree\n'
    expect(replay(base, local, server)).toEqual({ text: 'zero\none\ntwo and more\nthree\n', clean: true })
  })
  it('never duplicates the text', () => {
    const base = 'hello world'
    const r = replay(base, base, 'hello world')
    expect(r.text).toBe('hello world')
  })
})

describe('minimalEdit', () => {
  it('finds the changed middle', () => {
    expect(minimalEdit('abcdef', 'abXYef')).toEqual({ at: 2, remove: 2, insert: 'XY' })
  })
  it('never splits an emoji', () => {
    const e = minimalEdit('a😀b', 'a😃b')
    expect(e.at).toBe(1)
    expect(e.remove).toBe(2)
    expect(e.insert).toBe('😃')
  })
})
