/** A table wider than its card wraps its long text before it scrolls. */
import { expect, test } from 'vitest'
import { shrinkToFit } from '../src/tables/shrink'

const sum = (a: number[]) => a.reduce((x, y) => x + y, 0)

test('a table that fits is left alone', () => {
  expect(shrinkToFit([200, 64, 113], [110, 64, 113], 500)).toEqual([200, 64, 113])
})

test('a three-column table on a phone card gives the width back from the long text column only', () => {
  const out = shrinkToFit([194, 64, 113], [110, 64, 113], 325)
  expect(sum(out)).toBeCloseTo(325, 5)
  expect(out[1]).toBe(64)
  expect(out[2]).toBe(113)
  expect(out[0]).toBeGreaterThanOrEqual(110)
})

test('columns never go below their floors; the table then scrolls', () => {
  const out = shrinkToFit([194, 90, 113], [110, 80, 113], 200)
  expect(out).toEqual([110, 80, 113])
})

test('the share given back follows the slack above each floor', () => {
  const out = shrinkToFit([300, 200, 100], [100, 100, 100], 400)
  expect(out[2]).toBe(100)
  expect(300 - out[0]!).toBeCloseTo(2 * (200 - out[1]!), 5)
})
