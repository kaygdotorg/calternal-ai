import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { SyncClient } from '../../src/sync/client'
import { ImeGate } from '../../src/sync/gate'
import { memoryTransport } from '../../test-server/memory'
import { Room } from '../../test-server/room'
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 5000) { const end = Date.now() + ms; while (!f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) } }
describe('review4 resend', () => {
  it('R4-RESEND: a slow receipt does not repeat the paste upload', async () => {
    const room = new Room('start')
    const ch = `r4r-${Math.random()}`
    const mk = () => {
      const doc = new Y.Doc(); const transport = memoryTransport(() => room)
      const send = transport.send.bind(transport); let bytes = 0
      transport.send = (d) => { bytes += d.length; send(d) }
      const c = new SyncClient({ doc, transport, gate: new ImeGate(), channel: ch, lineage: () => room.lineage, checkDelayMs: 5, baseCheckDelayMs: 5 })
      return { doc, c, bytes: () => bytes }
    }
    const a = mk(), b = mk()
    try {
      a.c.connect(); b.c.connect()
      await until(() => a.c.isConfirmed && b.c.isConfirmed)
      room.checkDelayMs = 5000 // the typist receipt takes 5 s (slow uplink for a big paste)
      const b0 = b.bytes()
      a.doc.getText('md').insert(5, 'P'.repeat(5_000_000))
      await until(() => a.c.status.get().state === 'saved' && b.c.status.get().state === 'saved', 15000)
      const r = { a_uploaded: a.bytes(), b_uploaded_again: b.bytes() - b0 }
      console.log('R4-RESEND', JSON.stringify(r))
      expect(r.b_uploaded_again).toBeLessThan(10000)
    } finally { a.c.destroy(); b.c.destroy() }
  }, 20000)
})
