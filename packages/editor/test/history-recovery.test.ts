import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import * as encoding from 'lib0/encoding'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { Receipts } from '../src/sync/receipts'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms))
async function until(f: () => boolean, ms = 3000) {
  const end = Date.now() + ms
  while (!f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) }
}

describe('history recovery', () => {
  it('a retryable room error rejoins without persisting a refusal', async () => {
    const room = new Room('base')
    const doc = new Y.Doc()
    const transport = memoryTransport(() => room)
    let receive!: (packet: Uint8Array) => void
    const register = transport.onMessage.bind(transport)
    transport.onMessage = (fn) => { receive = fn; register(fn) }
    const refusals: Array<string | null> = []
    const client = new SyncClient({ doc, transport, gate: new ImeGate(), checkDelayMs: 5,
      onRefusalChange: (code) => refusals.push(code) })
    try {
      client.connect(); await until(() => client.isConfirmed)
      room.holdChecks = true
      doc.getText('md').insert(0, 'pending ')
      const e = encoding.createEncoder()
      encoding.writeVarUint(e, 103)
      encoding.writeVarUint8Array(e, new TextEncoder().encode(JSON.stringify('internal')))
      receive(encoding.toUint8Array(e))
      expect(transport.connected).toBe(false)
      expect(client.status.get().state).not.toBe('saved')
      expect(refusals.filter(Boolean)).toEqual([])
      room.releaseChecks()
      await until(() => client.isSynced && client.isConfirmed)
      expect(room.text).toBe('pending base')
    } finally { client.destroy(); doc.destroy() }
  })

  it('a local receipt permits Saved while later peer edits still hold back the safe base', () => {
    const r = new Receipts()
    r.receipt(r.check())
    const own = r.observe('me', 1, true)
    r.submitted(own)
    const check = r.check()
    const peer = r.observe('peer', 1, false)
    r.submitted(peer)
    r.receipt(check)
    expect(r.status(true).state).toBe('saved')
    expect(r.confirmed).toBe(false)
    expect(r.heldSince).not.toBeNull()
    r.destroy()
  })

  it('a retried refusal clears on its receipt without waiting for newer peer typing', () => {
    const r = new Receipts()
    r.observe('me', 1, true)
    r.refuse('too_large')
    r.submittedThrough(r.position, true)
    const check = r.check()
    const peer = r.observe('peer', 1, false)
    r.submitted(peer)
    r.receipt(check)
    expect(r.refusal).toBeNull()
    expect(r.status(true).state).toBe('saved')
    expect(r.confirmed).toBe(false)
    r.destroy()
  })

  it('clears a size refusal only after a new full submission has a covering receipt', async () => {
    const room = new Room('base')
    const doc = new Y.Doc()
    const client = new SyncClient({ doc, transport: memoryTransport(() => room), gate: new ImeGate(), checkDelayMs: 5 })
    try {
      client.connect(); await until(() => client.isConfirmed)
      room.holdChecks = true
      doc.getText('md').insert(0, 'oversize paste ')
      client.refuse('too_large')
      room.releaseChecks()
      await tick(30)
      expect(client.status.get().tooLarge).toBe(true)
      room.holdChecks = true
      doc.getText('md').delete(0, 'oversize paste '.length)
      await until(() => room.held.length > 0)
      expect(client.status.get().tooLarge).toBe(true)
      room.releaseChecks()
      await until(() => client.isConfirmed)
      expect(client.status.get().tooLarge).toBe(false)
      expect(room.text).toBe('base')
    } finally { client.destroy(); doc.destroy() }
  })

  it('a 10,000-edit receipt yields to the event loop before confirming', async () => {
    const doc = new Y.Doc()
    const r = new Receipts()
    let n = 0
    doc.on('update', (update) => r.observe('me', ++n, true, Date.now(), update))
    for (let i = 0; i < 10000; i++) doc.getText('md').insert(i, 'x')
    r.submittedThrough(r.position)
    const check = r.check()
    try {
      r.receipt(check)
      expect(r.confirmed).toBe(false)
      let yielded = false
      setTimeout(() => { yielded = true }, 0)
      await until(() => r.confirmed)
      expect(yielded).toBe(true)
    } finally { r.destroy(); doc.destroy() }
  })
})
