// Regression from the fourth review of #652. An edit over MAX_ROOM_MESSAGE is never sent; is it ever called Saved?
// MAX_ROOM_MESSAGE is lowered to 4 KiB here so the probe is fast; the client logic is unchanged.
import { describe, expect, it, vi } from 'vitest'
vi.mock('../../src/sync/protocol', async (orig) => ({ ...(await orig<typeof import('../../src/sync/protocol')>()), MAX_ROOM_MESSAGE: 4096 }))
import * as Y from 'yjs'
import { SyncClient } from '../../src/sync/client'
import { ImeGate } from '../../src/sync/gate'
import { describe as words } from '../../src/sync/status'
import { memoryTransport } from '../../test-server/memory'
import { Room } from '../../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 3000) {
  const end = Date.now() + ms
  while (!f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) }
}
const big = () => 'X'.repeat(8000)

function tab(room: Room, channel?: string, pendingSince?: number | null) {
  const doc = new Y.Doc()
  const transport = memoryTransport(() => room)
  let confirmations = 0
  const c = new SyncClient({ doc, transport, gate: new ImeGate(), channel, lineage: channel ? () => room.lineage : undefined,
    pendingSince, checkDelayMs: 5, baseCheckDelayMs: 5, onConfirmed: () => confirmations++ })
  return { doc, c, transport, text: doc.getText('md'), confirmations: () => confirmations }
}

describe('review4 too large', () => {
  it('R4-TOOBIG: one tab, an over-limit paste', async () => {
    const room = new Room('start')
    const a = tab(room)
    try {
      a.c.connect()
      await until(() => a.c.isSynced && a.c.isConfirmed)
      const before = a.confirmations()
      a.text.insert(a.text.length, big())
      const right_after = words(a.c.status.get())
      await tick(300)
      const r = { right_after, server_has_paste: room.text.length > 1000, state: a.c.status.get().state,
        words: words(a.c.status.get()), isConfirmed: a.c.isConfirmed, new_onConfirmed: a.confirmations() - before }
      console.log('R4-TOOBIG', JSON.stringify(r))
      expect(r.server_has_paste || (r.state !== 'saved' && !r.isConfirmed && r.new_onConfirmed === 0)).toBe(true)
    } finally { a.c.destroy() }
  })

  it('R4-TOOBIG-NEXT: a small edit after the over-limit paste', async () => {
    const room = new Room('start')
    const a = tab(room)
    try {
      a.c.connect()
      await until(() => a.c.isSynced && a.c.isConfirmed)
      a.text.insert(a.text.length, big())
      a.text.insert(a.text.length, ' <AFTER>')
      await tick(300)
      const r = { server_text_len: room.text.length, server_has_after: room.text.includes('<AFTER>'), state: a.c.status.get().state,
        words: words(a.c.status.get()), isConfirmed: a.c.isConfirmed }
      console.log('R4-TOOBIG-NEXT', JSON.stringify(r))
      expect(r.server_has_after || r.state !== 'saved').toBe(true)
    } finally { a.c.destroy() }
  })

  it('R4-TOOBIG-TAB: the other tab hears the typing tab\'s receipt', async () => {
    const room = new Room('start')
    const ch = `r4-${Math.random()}`
    const a = tab(room, ch), b = tab(room, ch)
    try {
      a.c.connect(); b.c.connect()
      await until(() => a.c.isSynced && b.c.isSynced && a.c.isConfirmed && b.c.isConfirmed)
      const before = b.confirmations()
      a.text.insert(a.text.length, big())
      await until(() => b.text.length > 1000)
      await tick(400)
      const r = { server_has_paste: room.text.length > 1000, a_state: a.c.status.get().state, b_state: b.c.status.get().state,
        b_isConfirmed: b.c.isConfirmed, b_new_onConfirmed: b.confirmations() - before }
      console.log('R4-TOOBIG-TAB', JSON.stringify(r))
      expect(r.server_has_paste || (r.b_state !== 'saved' && !r.b_isConfirmed)).toBe(true)
    } finally { a.c.destroy(); b.c.destroy() }
  })

  it('R4-TOOBIG-STEP2: an over-limit offline edit from an earlier visit, then reconnect', async () => {
    const room = new Room('')
    const doc = new Y.Doc()
    doc.getText('md').insert(0, big())
    const transport = memoryTransport(() => room)
    let confirmations = 0
    const c = new SyncClient({ doc, transport, gate: new ImeGate(), pendingSince: 1000, checkDelayMs: 5, baseCheckDelayMs: 5, onConfirmed: () => confirmations++ })
    try {
      c.connect()
      await until(() => c.isSynced)
      await tick(300)
      const r = { server_has_text: room.text.length > 1000, state: c.status.get().state, words: words(c.status.get()), isConfirmed: c.isConfirmed, onConfirmed: confirmations }
      console.log('R4-TOOBIG-STEP2', JSON.stringify(r))
      expect(r.server_has_text || (r.state !== 'saved' && !r.isConfirmed)).toBe(true)
    } finally { c.destroy() }
  })
})
