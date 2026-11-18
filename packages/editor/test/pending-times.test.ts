/** Timestamp ordering must survive arbitrary receipt order and clocks moving backwards. */
import { expect, it } from 'vitest'
import { PendingTimes, type PendingTime } from '../src/sync/pending-times'

it('keeps the oldest live timestamp through shuffled removals, including equal timestamps', () => {
  const times = new PendingTimes()
  const entries = [9, 4, 7, 1, 4, 2, 8, 0, 6, 3].map((at) => ({ at, timeSlot: -1 }))
  const live = new Set<PendingTime>()
  expect(times.oldest).toBeNull()
  for (const entry of entries) {
    times.add(entry); live.add(entry)
    expect(times.oldest).toBe(Math.min(...[...live].map((e) => e.at)))
  }
  for (const index of [3, 1, 7, 9, 0, 5, 8, 2, 4, 6]) {
    const entry = entries[index]!
    times.remove(entry); live.delete(entry)
    expect(entry.timeSlot).toBe(-1)
    expect(times.oldest).toBe(live.size ? Math.min(...[...live].map((e) => e.at)) : null)
  }
})

it('matches an independent minimum over 5,000 interleaved additions and retirements', () => {
  const times = new PendingTimes(), live = new Set<PendingTime>()
  let seed = 1074
  /** Reproducible choices exercise both sift directions without a wall-clock gate. */
  const random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed }
  for (let step = 0; step < 5000; step++) {
    if (!live.size || (live.size < 50 && random() % 3 !== 0)) {
      const entry = { at: random() % 1000, timeSlot: -1 }
      times.add(entry); live.add(entry)
    } else {
      const entry = [...live][random() % live.size]!
      times.remove(entry); live.delete(entry)
    }
    expect(times.oldest).toBe(live.size ? Math.min(...[...live].map((e) => e.at)) : null)
  }
  for (const entry of live) times.remove(entry)
  expect(times.oldest).toBeNull()
})
