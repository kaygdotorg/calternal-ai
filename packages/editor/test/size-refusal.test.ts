import { expect, it, vi } from 'vitest'
import * as Y from 'yjs'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { NoteSession } from '../src/note'
import { memoryMetaStore } from '../src/offline/meta'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

// Exercise the real size guard and real Yjs garbage collection with small fixtures.
vi.mock('../src/sync/protocol', async (original) => ({
  ...await original<typeof import('../src/sync/protocol')>(), MAX_ROOM_MESSAGE: 1024,
}))
const tick = (ms = 0) => new Promise((resolve) => setTimeout(resolve, ms))
async function until(f: () => boolean | Promise<boolean>, description = 'condition') {
  const end = Date.now() + 4000
  while (!await f()) { if (Date.now() > end) throw new Error(`timed out waiting for ${description}`); await tick(5) }
}

for (const disconnected of [false, true]) it(`removing an actually refused paste recovers (dropped=${disconnected})`, async () => {
  const room = new Room('base')
  const doc = new Y.Doc()
  const t = memoryTransport(() => room)
  const c = new SyncClient({ doc, transport: t, gate: new ImeGate(), checkDelayMs: 5 })
  try {
    c.connect(); await until(() => c.isConfirmed)
    doc.getText('md').insert(0, 'x'.repeat(2000))
    expect(c.status.get().tooLarge).toBe(true)
    expect(room.text).toBe('base')
    if (disconnected) { t.drop(); await tick() }
    room.holdChecks = true
    doc.getText('md').delete(0, 2000)
    await until(() => room.held.length > 0)
    expect(c.status.get().tooLarge).toBe(true)
    room.releaseChecks()
    await until(() => c.isConfirmed)
    expect(c.status.get().tooLarge).toBe(false)
    expect(room.text).toBe('base')
  } finally { c.destroy(); doc.destroy() }
})

for (const disconnected of [false, true]) it(`removing an undo-retained refused paste recovers without clearing undo (dropped=${disconnected})`, async () => {
  const room = new Room('base')
  const doc = new Y.Doc()
  const text = doc.getText('md')
  const undo = new Y.UndoManager(text)
  const t = memoryTransport(() => room)
  const c = new SyncClient({ doc, transport: t, gate: new ImeGate(), checkDelayMs: 5 })
  try {
    c.connect(); await until(() => c.isConfirmed)
    room.holdChecks = true
    text.insert(0, 'x'.repeat(2000))
    undo.stopCapturing()
    expect(c.status.get().tooLarge).toBe(true)
    expect(room.text).toBe('base')
    if (disconnected) { t.drop(); await tick() }
    const checksBeforeRemoval = room.checksReceived
    text.delete(0, 2000)
    // The real editor's UndoManager keeps the deleted insertion alive. A
    // full update of this document therefore still exceeds the wire limit.
    expect(text.toString()).toBe('base')
    expect(Y.encodeStateAsUpdate(doc).length).toBeGreaterThan(1024)
    expect(undo.undoStack).toHaveLength(2)
    await until(() => room.checksReceived > checksBeforeRemoval && room.held.length > 0 &&
      Y.snapshotContainsUpdate(Y.snapshot(room.doc), Y.encodeStateAsUpdate(doc)), 'fresh check covering the removed undo-retained paste')
    expect(c.status.get().tooLarge).toBe(true)
    expect(c.status.get().state).not.toBe('saved')
    expect(c.isConfirmed).toBe(false)
    room.releaseChecks()
    await until(() => c.isConfirmed, 'fresh durable receipt')
    expect(c.status.get().state).toBe('saved')
    expect(c.status.get().tooLarge).toBe(false)
    expect(room.text).toBe('base')
    expect(undo.undoStack).toHaveLength(2)
    expect(undo.redoStack).toHaveLength(0)
    expect(Y.encodeStateAsUpdate(doc).length).toBeGreaterThan(1024)
    undo.undo()
    expect(text.toString()).toBe(`${'x'.repeat(2000)}base`)
    expect(undo.undoStack).toHaveLength(1)
    expect(undo.redoStack).toHaveLength(1)
    expect(c.status.get().tooLarge).toBe(true)
    expect(c.status.get().state).not.toBe('saved')
    expect(room.text).toBe('base')
  } finally { c.destroy(); undo.destroy(); doc.destroy() }
})

it('clears the persisted refusal and synchronous storage marker only after a receipt', async () => {
  const room = new Room('base')
  const meta = memoryMetaStore()
  const id = crypto.randomUUID()
  const opts = { id, offline: false, meta, checkDelayMs: 5, transport: () => memoryTransport(() => room) }
  const s = await NoteSession.open(opts)
  const undo = new Y.UndoManager(s.text)
  let reopened: NoteSession | undefined
  try {
    s.connect(); await until(() => s.client.isConfirmed)
    s.text.insert(0, 'x'.repeat(2000))
    undo.stopCapturing()
    await until(async () => (await meta.get(id)).refused === 'too_large')
    expect(localStorage.getItem(`calternal-note-${id}:refused`)).toBe('too_large')
    room.holdChecks = true
    const checksBeforeRemoval = room.checksReceived
    s.text.delete(0, 2000)
    expect(Y.encodeStateAsUpdate(s.doc).length).toBeGreaterThan(1024)
    await until(() => room.checksReceived > checksBeforeRemoval && room.held.length > 0 &&
      Y.snapshotContainsUpdate(Y.snapshot(room.doc), Y.encodeStateAsUpdate(s.doc)), 'fresh check for persisted undo-retained refusal')
    expect((await meta.get(id)).refused).toBe('too_large')
    expect(localStorage.getItem(`calternal-note-${id}:refused`)).toBe('too_large')
    room.releaseChecks()
    await until(async () => s.client.isConfirmed && (await meta.get(id)).refused === null)
    expect(localStorage.getItem(`calternal-note-${id}:refused`)).toBeNull()
    expect(undo.undoStack).toHaveLength(2)
    undo.undo()
    expect(s.text.toString()).toBe(`${'x'.repeat(2000)}base`)
    await until(async () => (await meta.get(id)).refused === 'too_large')
    expect(localStorage.getItem(`calternal-note-${id}:refused`)).toBe('too_large')
    // Redo the removal before verifying the cleared marker survives reopen.
    undo.redo()
    await until(async () => s.client.isConfirmed && (await meta.get(id)).refused === null)
    expect(localStorage.getItem(`calternal-note-${id}:refused`)).toBeNull()
    s.destroy()
    reopened = await NoteSession.open(opts)
    reopened.connect(); await until(() => reopened!.client.isConfirmed)
    expect(reopened.status.get().tooLarge).toBe(false)
  } finally { undo.destroy(); s.destroy(); reopened?.destroy() }
})
