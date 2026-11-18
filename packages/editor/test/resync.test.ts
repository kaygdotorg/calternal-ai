// #652 round-7 review N1: the room's gap rule (resend asked, save checks held
// until a sync step 2 fills the gap) with a refused paste. The test room
// follows crates/notes exactly (test-server/resync.ts).
import { expect, it, vi } from 'vitest'
import * as Y from 'yjs'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { encodeUpdate } from '../src/sync/protocol'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'
import { followsOn } from '../test-server/resync'

vi.mock('../src/sync/protocol', async (original) => ({
  ...await original<typeof import('../src/sync/protocol')>(), MAX_ROOM_MESSAGE: 1024,
}))
const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms))
async function until(f: () => boolean, description: string) {
  const end = Date.now() + 4000
  while (!f()) { if (Date.now() > end) throw new Error(`timed out waiting for ${description}`); await tick(5) }
}

for (const keys of [1, 10]) for (const offline of [false, true]) {
  it(`typing ${keys} key(s) after a refused paste, then undoing it ${offline ? 'offline' : 'online'}, returns to Saved`, async () => {
    const room = new Room('base')
    const doc = new Y.Doc()
    const text = doc.getText('md')
    const undo = new Y.UndoManager(text)
    const t = memoryTransport(() => room)
    const c = new SyncClient({ doc, transport: t, gate: new ImeGate(), checkDelayMs: 5 })
    try {
      c.connect(); await until(() => c.isConfirmed, 'initial Saved')
      text.insert(text.length, 'P'.repeat(2000)); undo.stopCapturing()
      expect(c.status.get().tooLarge).toBe(true)
      // The keys follow the never-sent paste: the room sees a gap and asks for a resend.
      for (let i = 0; i < keys; i++) { text.insert(0, 'k'); await tick(10) }
      undo.stopCapturing()
      expect(room.text).toBe('base')
      expect(c.status.get().state).not.toBe('saved')
      if (offline) { t.drop(); await tick() }
      while (text.toString().includes('P')) undo.undo()
      if (offline) t.connect()
      await until(() => c.status.get().state === 'saved', 'Saved after undo')
      expect(c.status.get().tooLarge).toBe(false)
      expect(room.text).toBe(text.toString())
    } finally { c.destroy(); undo.destroy(); doc.destroy() }
  })
}

it('the test room applies the gap rule like crates/notes', () => {
  const room = new Room('base')
  const writer = new Y.Doc()
  const updates: Uint8Array[] = []
  writer.on('update', (u: Uint8Array) => updates.push(u))
  writer.getText('md').insert(0, 'a')
  writer.getText('md').insert(1, 'b')
  expect(followsOn(room.doc, updates[1]!)).toBe(false)
  expect(followsOn(room.doc, updates[0]!)).toBe(true)
  const sent: Uint8Array[] = []
  const conn = { send: (d: Uint8Array) => sent.push(d), close() {} }
  room.join(conn, null)
  sent.length = 0
  room.receive(conn, encodeUpdate(updates[1]!))
  expect(room.text).toBe('base')
  expect(sent).toHaveLength(1) // a sync step 1: resend what I lack
  writer.destroy()
})

it('a forwarded sibling edit the room refused for a gap is not Saved by a resend that lacks it', async () => {
  const room = new Room('base')
  const doc = new Y.Doc()
  const gate = new ImeGate()
  const t = memoryTransport(() => room)
  const c = new SyncClient({ doc, transport: t, gate, checkDelayMs: 5 })
  const sibling = new Y.Doc()
  try {
    c.connect(); await until(() => c.isConfirmed, 'initial Saved')
    Y.applyUpdate(sibling, Y.encodeStateAsUpdate(doc))
    // The sibling's first edit never reached the room; its second one depends on it.
    sibling.getText('md').insert(0, 'first ')
    let second!: Uint8Array
    sibling.once('update', (u: Uint8Array) => { second = u })
    sibling.getText('md').insert(0, 'second ')
    // Held behind IME here, and forwarded because the sibling is offline.
    gate.start()
    ;(c as unknown as { onTab(m: unknown): void }).onTab({ t: 'u', u: second, off: true, from: 999, n: 2, l: null })
    await tick(200)
    // The room refused it for the gap and asked for a resend; the answer could
    // only hold what is applied here, so the edit is not Saved.
    expect(room.text).toBe('base')
    expect(c.status.get().state).not.toBe('saved')
  } finally { gate.end(); c.destroy(); doc.destroy(); sibling.destroy() }
})

it('an answer to a check queued before the gap does not end the wait for a resend', async () => {
  // Review 8 R8-1. The room answers checks queued before a gap while it waits
  // for the resend. Such an answer must not make this client treat later
  // plain updates as accepted: a sibling edit forwarded meanwhile, still
  // behind IME here, would lose its "send again" mark and be retired by the
  // answer to a check after a step 2 that lacks it.
  const { decodeSaveCheck, encodeSaved, encodeStep1, encodeStep2 } = await import('../src/sync/protocol')
  const doc = new Y.Doc()
  const gate = new ImeGate()
  let message!: (data: Uint8Array) => void
  let open!: () => void
  let connected = false
  const packets: Uint8Array[] = []
  const transport = {
    get connected() { return connected },
    connect() { connected = true; open() }, disconnect() { connected = false },
    send(data: Uint8Array) { packets.push(data) },
    onOpen(f: () => void) { open = f }, onClose() {},
    onMessage(f: (data: Uint8Array) => void) { message = f },
  }
  const c = new SyncClient({ doc, transport, gate, channel: null, checkDelayMs: 1_000_000 })
  const check = () => (c as unknown as { sendCheck(): void }).sendCheck()
  const lastCheck = () => { for (const p of [...packets].reverse()) { const id = decodeSaveCheck(p); if (id != null) return id } throw new Error('no check') }
  const server = new Y.Doc()
  const sibling = new Y.Doc()
  try {
    c.connect(); message(encodeStep1(server)); message(encodeStep2(server, Y.encodeStateVector(doc)))
    check(); message(encodeSaved(lastCheck()))
    expect(c.status.get().state).toBe('saved')
    Y.applyUpdate(server, Y.encodeStateAsUpdate(doc)); Y.applyUpdate(sibling, Y.encodeStateAsUpdate(doc))
    const text = doc.getText('md')
    text.insert(0, 'P'.repeat(2000))
    expect(c.status.get().tooLarge).toBe(true)
    check(); const queued = lastCheck() // queued at the room before the gap
    text.insert(0, 'k') // a gap at the room: it asks for a resend
    message(encodeStep1(server))
    // Too large to answer with a step 2. The room still answers the queued check.
    message(encodeSaved(queued))
    // A sibling's edit arrives while composing and is forwarded (the sibling is offline).
    gate.start()
    let edit!: Uint8Array
    sibling.once('update', (u: Uint8Array) => { edit = u })
    sibling.getText('md').insert(0, 'sibling ')
    ;(c as unknown as { onTab(m: unknown): void }).onTab({ t: 'u', u: edit, off: true, from: 999, n: 1, l: null })
    // Deleting the paste sends the complete state, a step 2 without the held edit,
    // which satisfies the room; it then answers the next check.
    text.delete(1, 2000)
    check()
    message(encodeSaved(lastCheck()))
    await tick(10)
    expect(c.status.get().state).not.toBe('saved')
  } finally { gate.end(); c.destroy(); doc.destroy(); server.destroy(); sibling.destroy() }
})
