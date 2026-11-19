import { describe, expect, it } from 'vitest'
import { formatDay } from '../src/dates'

describe('formatDay', () => {
  const now = new Date(2026, 9, 13)
  it('reads "Wednesday, 7 October" in the current year', () => {
    expect(formatDay(new Date(2026, 9, 7), now)).toBe('Wednesday, 7 October')
  })
  it('adds the year only for other years', () => {
    expect(formatDay(new Date(2025, 9, 6), now)).toBe('Monday, 6 October 2025')
    expect(formatDay(new Date(2027, 0, 1), now)).toBe('Friday, 1 January 2027')
  })
})
