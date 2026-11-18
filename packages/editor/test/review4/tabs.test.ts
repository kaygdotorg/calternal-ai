// Regression from the fourth review of #652. Sibling-tab receipts.
import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { SyncClient } from '../../src/sync/client'
import { ImeGate } from '../../src/sync/gate'
import { memoryTransport } from '../../test-server/memory'
import { Room } from '../../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 3000) {
  const end = Date.now() + ms
  while (!f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) }
}

function tab(room: Room, channel: string, o: { stall?: boolean; lateLineage?: boolean } = {}) {
  const doc = new Y.Doc()
  const transport = memoryTransport(() => room)
  const send = transport.send.bind(transport)
  let stuck = false
  transport.send = (d) => { if (!(o.stall && stuck)) send(d) }
  let opened = false
  transport.onOpen(() => (opened = true))
  const atConfirm: boolean[] = []
  const c = new SyncClient({ doc, transport, gate: new ImeGate(), channel,
    // lateLineage: like a tab that learns the lineage only from its join answer (first open on this device).
    lineage: () => (o.lateLineage && !opened ? null : room.lineage),
    checkDelayMs: 5, baseCheckDelayMs: 5, onConfirmed: () => { if (doc.getText('md').toString().includes('<C>')) atConfirm.push(room.text.includes('<C>')) } })
  return { doc, c, text: doc.getText('md'), stick: () => (stuck = true), atConfirm }
}

describe('review4 tabs', () => {
  it('R4-JOIN: a joining tab waits for coverage of a third tab\'s edit', async () => {
    const room = new Room('start')
    const ch = `r4j-${Math.random()}`
    const a = tab(room, ch)
    const c = tab(room, ch, { stall: true })
    try {
      a.c.connect(); c.c.connect()
      await until(() => a.c.isSynced && c.c.isSynced && a.c.isConfirmed && c.c.isConfirmed)
      // C's socket is stuck (connected, frames lost); C types and is then closed before any receipt.
      c.stick()
      c.text.insert(c.text.length, ' <C>')
      await until(() => a.text.toString().includes('<C>'))
      c.c.destroy()
      // A holds C's edit for up to 2 s (longer if A's timers are throttled in the background).
      const d = tab(room, ch, { lateLineage: true })
      d.c.connect()
      await until(() => d.text.toString().includes('<C>'), 1500)
      await until(() => d.c.isConfirmed || room.text.includes('<C>'), 1500).catch(() => {})
      const r = { server_has_C: room.text.includes('<C>'), d_isConfirmed: d.c.isConfirmed, d_state: d.c.status.get().state,
        d_onConfirmed_without_C_on_server: d.atConfirm.filter((x) => !x).length, a_isConfirmed: a.c.isConfirmed }
      console.log('R4-JOIN', JSON.stringify(r))
      await until(() => room.text.includes('<C>'), 4000)
      console.log('R4-JOIN later: server has C after A adopted it =', room.text.includes('<C>'))
      d.c.destroy()
      expect(r.d_onConfirmed_without_C_on_server).toBe(0)
    } finally { a.c.destroy(); c.c.destroy() }
  })

  it('R4-CLOSE3: three tabs, the typist closes mid-receipt', async () => {
    const room = new Room('start')
    const ch = `r4c-${Math.random()}`
    const a = tab(room, ch), b = tab(room, ch), c = tab(room, ch, { stall: true })
    try {
      for (const t of [a, b, c]) t.c.connect()
      await until(() => [a, b, c].every((t) => t.c.isSynced && t.c.isConfirmed))
      c.stick()
      const a0 = a.atConfirm.length, b0 = b.atConfirm.length
      c.text.insert(c.text.length, ' <C>')
      await until(() => a.text.toString().includes('<C>') && b.text.toString().includes('<C>'))
      c.c.destroy()
      const early = { a: a.c.isConfirmed, b: b.c.isConfirmed, sa: a.c.status.get().state, sb: b.c.status.get().state }
      await until(() => room.text.includes('<C>') && a.c.isConfirmed && b.c.isConfirmed, 5000)
      const starts = room.text.split('<C>').length - 1
      const r = { early, server_C_count: starts, a_bad: a.atConfirm.slice(a0).filter((x) => !x).length, b_bad: b.atConfirm.slice(b0).filter((x) => !x).length }
      console.log('R4-CLOSE3', JSON.stringify(r))
      expect(r.server_C_count).toBe(1)
      expect(r.a_bad + r.b_bad).toBe(0)
    } finally { a.c.destroy(); b.c.destroy(); c.c.destroy() }
  })

  it('R4-GARBLED: malformed edits are ignored and forged durability never confirms state', async () => {
    const room = new Room('start')
    const ch = `r4g-${Math.random()}`
    const a = tab(room, ch)
    try {
      a.c.connect()
      await until(() => a.c.isSynced && a.c.isConfirmed)
      const other = new Y.Doc()
      Y.applyUpdate(other, Y.encodeStateAsUpdate(a.doc))
      let u: Uint8Array | null = null
      other.on('update', (x: Uint8Array) => (u = x))
      other.getText('md').insert(0, '<G>')
      const bc = new BroadcastChannel(ch)
      bc.postMessage({ t: 'u', u, from: 42, n: 'x', ok: 0, l: room.lineage })
      await tick(100)
      expect(a.text.toString()).not.toContain('<G>')
      expect(room.text).not.toContain('<G>')
      await tick(300)
      const r = { server_has_G: room.text.includes('<G>'), a_state: a.c.status.get().state, a_isConfirmed: a.c.isConfirmed }
      console.log('R4-GARBLED', JSON.stringify(r))
      // forged durable claim
      const other2 = new Y.Doc(); Y.applyUpdate(other2, Y.encodeStateAsUpdate(a.doc))
      let u2: Uint8Array | null = null
      other2.on('update', (x: Uint8Array) => (u2 = x))
      other2.getText('md').insert(0, '<F>')
      bc.postMessage({ t: 'ok', from: 77, n: 1e9, l: room.lineage })
      bc.postMessage({ t: 'u', u: u2, from: 77, n: 1, ok: 0, l: room.lineage })
      await until(() => a.text.toString().includes('<F>'))
      await tick(100)
      expect(room.text).not.toContain('<F>')
      expect(a.c.isConfirmed).toBe(false)
      expect(a.c.status.get().state).not.toBe('saved')
      await until(() => room.text.includes('<F>') && a.c.isConfirmed, 5000)
      const f = { server_has_F: room.text.includes('<F>'), a_state: a.c.status.get().state, a_isConfirmed: a.c.isConfirmed }
      console.log('R4-FORGED', JSON.stringify(f))
      expect(f.server_has_F || !f.a_isConfirmed).toBe(true)
      bc.close()
    } finally { a.c.destroy() }
  }, 10000)
})
