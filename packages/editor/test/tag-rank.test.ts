/** Tag correctness at small and 10k sizes; timing qualification lives in perf/receipt-rank.ts. */
import { describe, expect, it } from 'vitest'
import { matchTags, rankTags, tagIndex } from '../src/transforms/tag-rank'
import { linkKind } from '../src/syntax/links'

// Every tag and every tag above a nested one, with its notes (as the app counts them).
const counts = new Map<string, number>([
  ['area', 9],
  ['area/work', 6],
  ['area/home', 4],
  ['area/fitness', 2],
  ['fitness', 1],
  ['goals', 3],
  ['goals/fitness', 3],
  ['goals/fitness/strength', 2],
  ['art', 2],
  ['archive', 12],
  ['reading', 3],
  ['reading/fiction', 3],
  ['recipes', 5],
  ['Rcp-old', 1],
])
const tags = (q: string, limit = 20) => rankTags(counts, q, limit).map((r) => r.tag)

describe('the tag pickers (#449)', () => {
  it('puts names that start with what was typed first, by use', () => {
    expect(tags('ar')).toEqual(['archive', 'area', 'area/work', 'area/home', 'area/fitness', 'art'])
    expect(tags('rcp')).toEqual(['Rcp-old'])
  })
  it('finds a tag by any part of its path', () => {
    expect(tags('fit')).toEqual(['fitness', 'goals/fitness', 'area/fitness', 'goals/fitness/strength'])
    expect(tags('stre')).toEqual(['goals/fitness/strength'])
  })
  it('ranks: exact, path start, exact last part, other part, then notes, then A to Z', () => {
    // `fitness` itself; no path starts with it but `fitness`'s own; then last part exactly
    // (`goals/fitness` has more notes than `area/fitness`); then `goals/fitness/strength`, where it is a middle part.
    expect(tags('fitness')).toEqual(['fitness', 'goals/fitness', 'area/fitness', 'goals/fitness/strength'])
    // A path start beats an exact last part.
    expect(tags('area')).toEqual(['area', 'area/work', 'area/home', 'area/fitness'])
    // Ties by notes go A to Z.
    expect(tags('reading/f')).toEqual(['reading/fiction'])
    const tie = new Map([['b/x', 1], ['a/x', 1], ['c/x', 1]])
    expect(rankTags(tie, 'x').map((r) => r.tag)).toEqual(['a/x', 'b/x', 'c/x'])
  })
  it('a query with a slash may start at any part', () => {
    expect(tags('fitness/s')).toEqual(['goals/fitness/strength'])
    expect(tags('area/h')).toEqual(['area/home'])
    expect(tags('AREA/W')).toEqual(['area/work'])
    // Never in the middle of a part.
    expect(tags('ness/s')).toEqual([])
  })
  it('says which tags hold more, and which letters matched', () => {
    const top = rankTags(counts, 'are')
    expect(top[0]).toEqual({ tag: 'area', count: 9, kids: true, match: [0, 3] })
    expect(rankTags(counts, 'fiction')[0]).toEqual({ tag: 'reading/fiction', count: 3, kids: false, match: [8, 15] })
    expect(rankTags(counts, 'strength')[0]!.match).toEqual([14, 22])
  })
  it('keeps only the best few, and leaves out what the note has', () => {
    expect(tags('a', 3)).toEqual(['archive', 'area', 'area/work'])
    expect(matchTags(tagIndex(counts), 'ar', 7, new Set(['archive', 'area'])).map((r) => r.tag)[0]).toBe('area/work')
  })
  it('offers nothing for a tag nobody has used', () => {
    expect(tags('zebra')).toEqual([])
  })
  it('keeps matching bounded and deterministic at ten thousand tags', () => {
    const big = new Map<string, number>()
    for (let i = 0; i < 10_000; i++) big.set(`area${i % 50}/topic${i % 700}/item-${i}`, (i * 7) % 31)
    const ix = tagIndex(big)
    const typed = ['a', 'ar', 'top', 'topic12', 'item-99', 'area3/topic4', 'zzz']
    for (const query of typed) {
      const rows = matchTags(ix, query)
      expect(rows.length).toBeLessThanOrEqual(7)
      expect(rows).toEqual(matchTags([...ix].reverse(), query))
    }
    expect(matchTags(ix, 'zzz')).toEqual([])
    expect(matchTags(ix, 'item-9999').map((row) => row.tag)).toEqual(['area49/topic199/item-9999'])
  })
})

describe('link kinds', () => {
  it('names what a link points at', () => {
    expect(linkKind('https://example.org')).toBe('web')
    expect(linkKind('Trip ideas.md')).toBe('note')
    expect(linkKind('Journal/2026-10-15.md')).toBe('day')
    expect(linkKind('2026-W42.md')).toBe('day')
    expect(linkKind('Photos/Beach%20day.JPG')).toBe('photo')
    expect(linkKind('Files/report.pdf')).toBe('file')
    expect(linkKind('Note.md#^a1b2')).toBe('note')
  })
})
