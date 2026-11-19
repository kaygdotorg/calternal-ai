/**
 * Corruption traps from issue #3 that live on this side: what happens to a
 * device's copy when the server's copy starts a new lineage (restored from
 * a backup, re-seeded from the file), with collaborators and other tabs.
 */

import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { NoteSession } from '../src/note'
import { memoryMetaStore } from '../src/offline/meta'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { type MemoryTransport, memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean | Promise<boolean>, ms = 3000) {
  const end = Date.now() + ms
  while (!(await f())) {
    if (Date.now() > end) throw new Error('timed out')
    await tick(5)
  }
}
const count = (s: string, w: string) => s.split(w).length - 1

async function open(room: Room, id: string, meta = memoryMetaStore()) {
  let transport!: MemoryTransport
  let copy: string | null = null
  const s = await NoteSession.open({
    id,
    offline: false,
    meta,
    checkDelayMs: 5,
    transport: (lineage, learn) => (transport = memoryTransport(() => room, lineage, learn)),
    onConflictCopy: (t) => (copy = t),
  })
  s.connect()
  await until(() => s.client.isSynced && s.status.get().state === 'saved')
  return { s, meta, transport: () => transport, copy: () => copy }
}

describe('a new server lineage', () => {
  it('does not double what a collaborator typed after my last save', async () => {
    const id = `n-${Math.random()}`
    const room = new Room('mine\n')
    const a = await open(room, id)
    // A collaborator types; this device receives it but saves nothing new.
    const doc = new Y.Doc()
    const b = new SyncClient({ doc, transport: memoryTransport(() => room), gate: new ImeGate(), checkDelayMs: 5 })
    b.connect()
    await until(() => b.isSynced)
    doc.getText('md').insert(doc.getText('md').length, 'theirs\n')
    await until(() => a.s.text.toString() === 'mine\ntheirs\n')
    // Once durable, the collaborator's line joins this device's replay base.
    await until(async () => (await a.meta.get(id)).base === 'mine\ntheirs\n')
    b.destroy()

    // Offline, this device adds a line; the server is restored from its
    // file, which already has the collaborator's line.
    a.transport().drop()
    await tick(5)
    a.s.text.insert(a.s.text.length, 'offline\n')
    room.seed('mine\ntheirs\n')
    a.s.connect()
    await until(() => room.text.includes('offline'))
    await until(() => a.s.text.toString() === room.text)
    expect(room.text).toBe('mine\ntheirs\noffline\n')
    expect(a.copy()).toBeNull()
  })

  it('a tab still holding the old lineage never feeds it into the new one', async () => {
    const id = `n-${Math.random()}`
    const room = new Room('# Two tabs\nshared line\n')
    const t1 = await open(room, id)
    const t2 = await open(room, id)
    // Tab two goes quiet (laptop lid, background tab) still holding the
    // old copy; the server's copy is replaced; tab one recovers.
    t2.s.disconnect()
    await tick(5)
    room.seed('# Two tabs\nshared line\n')
    await until(() => t1.s.client.isSynced && t1.s.lineage === room.lineage)
    await tick(100)
    expect(count(room.text, 'shared line')).toBe(1)
    expect(count(t1.s.text.toString(), 'shared line')).toBe(1)
    // Typing in the old tab doesn't reach the new copy either.
    t2.s.text.insert(t2.s.text.length, 'old tab\n')
    await tick(50)
    expect(count(room.text, 'shared line')).toBe(1)
    expect(t1.s.text.toString()).toBe(room.text)
    t1.s.destroy()
    t2.s.destroy()
  })
})
