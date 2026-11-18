/**
 * Status publication belongs to the same task as a receipt drain. Reading
 * its timestamps must not walk the backlog after the drain has yielded:
 * that work would escape the drain's deadline and allocate iterator results.
 */
import { expect, it, vi } from 'vitest'
import { Receipts } from '../src/sync/receipts'

it('publishes exact pending times without scanning a 10,000-edit ledger between drain steps', () => {
  const callbacks: Array<() => void> = []
  /** Holds posted tasks so the assertion sees a partially drained backlog. */
  class Channel {
    port1 = { onmessage: null as (() => void) | null, close() {} }
    port2 = { postMessage: () => callbacks.push(() => this.port1.onmessage?.()), close() {} }
  }
  vi.stubGlobal('MessageChannel', Channel)
  const receipts = new Receipts()
  try {
    for (let i = 0; i < 10_000; i++) receipts.observe('writer', i, i % 3 !== 0, 10_000 - i)
    receipts.submittedThrough(receipts.position)
    const ledger = (receipts as unknown as { edits: Map<number, unknown> }).edits
    const values = ledger.values.bind(ledger)
    let visited = 0
    /** Counts ledger visits rather than depending on a shared VM's clock. */
    vi.spyOn(ledger, 'values').mockImplementation(function* () {
      for (const edit of values()) { visited++; yield edit }
      return undefined
    })
    receipts.receipt(receipts.check())
    expect(callbacks.length).toBeGreaterThan(0)
    expect(receipts.status(true).since).toBe(2)
    expect(receipts.heldSince).toBe(1)
    expect(visited).toBe(0)
    while (callbacks.length) {
      const before = ledger.size
      callbacks.shift()!()
      expect(before - ledger.size).toBeLessThanOrEqual(64)
      receipts.status(true); void receipts.heldSince
      expect(visited).toBe(0)
    }
    expect(receipts.confirmed).toBe(true)
    expect(receipts.pendingSince).toBeNull()
    expect(receipts.heldSince).toBeNull()
  } finally {
    receipts.destroy()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  }
})
