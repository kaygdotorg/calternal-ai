import { describe, expect, it } from 'vitest'
import { branchRange, CONTINUATION as MORE, OUTSIDE } from '../src/lists/branch'

// Which lines light up with an item (#247): its own line and everything under it.
//
//   0  Pack            depth 0
//   1    Passport      depth 1
//   2    Clothes       depth 1
//   3      Shirts      depth 2
//   4      (wrapped)   continuation
//   5      Socks       depth 2
//   6    Charger       depth 1
//   7  Leave           depth 0
//   8  (a paragraph)   outside
//   9  Other           depth 0
const depths = [0, 1, 1, 2, MORE, 2, 1, 0, OUTSIDE, 0]

describe('branch range', () => {
  it('takes an item and everything deeper under it', () => {
    expect(branchRange(depths, 0)).toEqual({ from: 0, to: 7, depth: 0 })
    expect(branchRange(depths, 2)).toEqual({ from: 2, to: 6, depth: 1 })
    expect(branchRange(depths, 5)).toEqual({ from: 5, to: 6, depth: 2 })
  })

  it('a last child ends where the parent does', () => {
    expect(branchRange(depths, 6)).toEqual({ from: 6, to: 7, depth: 1 })
  })

  it('a wrapped line belongs to the item above it', () => {
    expect(branchRange(depths, 4)).toEqual({ from: 3, to: 5, depth: 2 })
  })

  it('stops at the end of the list', () => {
    expect(branchRange(depths, 7)).toEqual({ from: 7, to: 8, depth: 0 })
    expect(branchRange(depths, 9)).toEqual({ from: 9, to: 10, depth: 0 })
  })

  it('says nothing for a line outside any list, or off the region', () => {
    expect(branchRange(depths, 8)).toBeNull()
    expect(branchRange(depths, -1)).toBeNull()
    expect(branchRange(depths, 10)).toBeNull()
    expect(branchRange([MORE, 0], 0)).toBeNull()
  })

  it('is a short walk even for a huge list', () => {
    const big = Array.from({ length: 100_000 }, (_, i) => i % 5)
    const t0 = performance.now()
    for (let i = 0; i < 200; i++) branchRange(big, 50_000 + i * 5)
    expect((performance.now() - t0) / 200).toBeLessThan(0.5)
  })
})
