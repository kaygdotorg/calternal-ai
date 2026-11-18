/**
 * Indexes live edit timestamps so publishing status never scans a backlog.
 * Each edit belongs to one heap and carries its current slot, avoiding an
 * extra id map. Updates cost O(log n); reading the oldest time costs O(1).
 */

/** A live timestamp; its owner must not change `at` while it is indexed. */
export interface PendingTime {
  readonly at: number
  /** Heap position, or -1 after retirement. Only PendingTimes changes it. */
  timeSlot: number
}

/** A minimum heap supporting retirement of any live edit, including timestamp ties. */
export class PendingTimes {
  private entries: PendingTime[] = []

  /** The exact oldest live time, even if the system clock moved backwards. */
  get oldest(): number | null { return this.entries[0]?.at ?? null }

  /** Adds a new edit; callers give each live edit to exactly one timestamp index. */
  add(entry: PendingTime): void {
    this.entries.push(entry)
    this.place(entry, this.entries.length - 1)
  }

  /** Retires an edit inside its receipt batch, so status has no deferred cleanup. */
  remove(entry: PendingTime): void {
    const at = entry.timeSlot, last = this.entries.pop()!
    entry.timeSlot = -1
    if (at < this.entries.length) this.place(last, at)
  }

  /** Moves a hole through the heap, updating displaced positions before filling it. */
  private place(entry: PendingTime, at: number): void {
    const entries = this.entries
    while (at > 0) {
      const parent = (at - 1) >>> 1, above = entries[parent]!
      if (above.at <= entry.at) break
      entries[at] = above; above.timeSlot = at
      at = parent
    }
    while (at * 2 + 1 < entries.length) {
      let child = at * 2 + 1
      if (child + 1 < entries.length && entries[child + 1]!.at < entries[child]!.at) child++
      const below = entries[child]!
      if (entry.at <= below.at) break
      entries[at] = below; below.timeSlot = at
      at = child
    }
    entries[at] = entry; entry.timeSlot = at
  }
}
