import { Text } from '@codemirror/state'
import { describe, expect, it } from 'vitest'
import { type LinkSpan, linkKind, linkSide } from '../src/syntax/links'

/** The links of `src` (`[words](target)`), as the editor finds them. */
function find(src: string): { doc: Text; links: LinkSpan[] } {
  const links: LinkSpan[] = []
  for (const m of src.matchAll(/\[([^\]]*)\]\(([^)]*)\)/g)) {
    const from = m.index!
    links.push({ from, to: from + m[0].length, textFrom: from + 1, textTo: from + 1 + m[1]!.length, href: m[2]!, kind: linkKind(m[2]!) })
  }
  return { doc: Text.of(src.split('\n')), links }
}
const sides = (src: string, i = 0) => {
  const { doc, links } = find(src)
  return [linkSide(doc, links, i, 'l'), linkSide(doc, links, i, 'r')]
}

describe('what stands beside a link, per side (#469)', () => {
  it('a space on a side is a space', () => {
    expect(sides('See [Ranisa](a.md) then')).toEqual(['sp', 'sp'])
    expect(sides('a\t[Ranisa](a.md) b')).toEqual(['sp', 'sp'])
  })
  it('a comma, a full stop, a bracket or a tag touching is tight', () => {
    expect(sides('See [Ranisa](a.md), then')).toEqual(['sp', 'tight'])
    expect(sides('(in brackets [Ranisa](a.md)).')).toEqual(['sp', 'tight'])
    expect(sides('([Ranisa](a.md)) end')).toEqual(['tight', 'tight'])
    expect(sides('[Ranisa](a.md)#area/x')).toEqual(['tight', 'tight'])
    expect(sides('a[Ranisa](a.md)b')).toEqual(['tight', 'tight'])
  })
  it('the start and the end of a line are tight', () => {
    expect(sides('[Ranisa](a.md) then')).toEqual(['tight', 'sp'])
    expect(sides('then [Ranisa](a.md)')).toEqual(['sp', 'tight'])
    expect(sides('one\n[Ranisa](a.md)\ntwo')).toEqual(['tight', 'tight'])
  })
  it('links touching are tight on the touching sides, and apart they are spaced', () => {
    expect(sides('[a](a.md)[b](b.md)', 0)).toEqual(['tight', 'tight'])
    expect(sides('[a](a.md)[b](b.md)', 1)).toEqual(['tight', 'tight'])
    expect(sides('x [a](a.md) [b](b.md) y', 0)).toEqual(['sp', 'sp'])
    expect(sides('x [a](a.md) [b](b.md) y', 1)).toEqual(['sp', 'sp'])
  })
  it('hidden markup around a link is looked through', () => {
    expect(sides('go **[Ranisa](a.md)** now')).toEqual(['sp', 'sp'])
    expect(sides('go **[Ranisa](a.md)**, now')).toEqual(['sp', 'tight'])
    expect(sides('~~[Ranisa](a.md)~~')).toEqual(['tight', 'tight'])
  })
})
