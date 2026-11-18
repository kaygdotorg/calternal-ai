import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import * as encoding from 'lib0/encoding'
import { NoteSession, pendingNotes } from '../src/note'
import { memoryMetaStore } from '../src/offline/meta'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { decodeSaveCheck, encodeSaved } from '../src/sync/protocol'
import { type MemoryTransport, memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 2000) {
  const end = Date.now() + ms
  while (!f()) {
    if (Date.now() > end) throw new Error('timed out')
    await tick(5)
  }
}

function client(room: Room, opts: Partial<{ gate: ImeGate }> = {}) {
  const doc = new Y.Doc()
  const transport = memoryTransport(() => room)
  const c = new SyncClient({ doc, transport, gate: opts.gate ?? new ImeGate(), checkDelayMs: 5, baseCheckDelayMs: 20 })
  c.connect()
  return { doc, c, transport, text: doc.getText('md') }
}

describe('sync client', () => {
  it('stale receipts cannot clear a refusal; a fresh resend and receipt can', async () => {
    const room = new Room('start')
    const doc = new Y.Doc()
    const transport = memoryTransport(() => room)
    let receive!: (data: Uint8Array) => void
    const register = transport.onMessage.bind(transport)
    transport.onMessage = (fn) => { receive = fn; register(fn) }
    let confirmations = 0
    const c = new SyncClient({ doc, transport, gate: new ImeGate(), checkDelayMs: 5, baseCheckDelayMs: 5,
      onConfirmed: () => { confirmations++ } })
    try {
      c.connect()
      await until(() => c.isConfirmed)
      room.holdChecks = true
      doc.getText('md').insert(0, 'refused ')
      await until(() => room.held.length > 0)
      const before = confirmations
      const packet = encoding.createEncoder()
      encoding.writeVarUint(packet, 102)
      encoding.writeVarUint8Array(packet, new TextEncoder().encode(JSON.stringify('storage_quota')))
      receive(encoding.toUint8Array(packet))
      room.releaseChecks()
      receive(encodeSaved(1_000_000))
      await tick(20)
      expect(c.status.get().state).toBe('not-saved')
      expect(c.status.get().error).toBe('Could not save')
      expect(confirmations).toBe(before)
      room.holdChecks = true
      transport.drop(); c.connect()
      await until(() => c.isSynced)
      await tick(40)
      expect(c.status.get().state).toBe('not-saved')
      expect(c.isConfirmed).toBe(false)
      expect(confirmations).toBe(before)
      room.releaseChecks()
      await until(() => c.isConfirmed)
      expect(c.status.get().state).toBe('saved')
      expect(c.status.get().error).toBeUndefined()
    } finally { c.destroy() }
  })

  it('a cached document waits for a server receipt without inventing a local pending marker', async () => {
    const room = new Room('server words')
    room.holdChecks = true
    const doc = new Y.Doc()
    doc.getText('md').insert(0, 'cached words')
    const transport = memoryTransport(() => room)
    const pending: Array<number | null> = []
        const c = new SyncClient({ doc, transport, gate: new ImeGate(), pendingSince: null,
      checkDelayMs: 5, baseCheckDelayMs: 5, onPendingChange: (s) => pending.push(s),
    })
    try {
      // No receipt yet: the document is unconfirmed, with no invented local edit.
      expect(c.status.get().state).toBe('not-saved')
      c.connect()
      await until(() => c.isSynced && room.held.length > 0)
      expect(c.status.get().state).toBe('saving')
      expect(pending.every((s) => s == null)).toBe(true)
      // The replay base still waits for a durable receipt.
      expect(c.isConfirmed).toBe(false)
      room.releaseChecks()
      await until(() => c.isConfirmed)
    } finally { c.destroy() }
  })

  it('unconfirmed edits from an earlier visit stay unsaved until a receipt', async () => {
    const room = new Room('')
    room.holdChecks = true
    const doc = new Y.Doc()
    doc.getText('md').insert(0, 'typed offline last time')
    const transport = memoryTransport(() => room)
    const c = new SyncClient({ doc, transport, gate: new ImeGate(), pendingSince: 1000, checkDelayMs: 5 })
    try {
      expect(c.status.get().state).toBe('not-saved')
      c.connect()
      await until(() => c.isSynced && room.held.length > 0)
      expect(c.status.get().state).toBe('saving')
      expect(room.text).toBe('typed offline last time')
      room.releaseChecks()
      await until(() => c.status.get().state === 'saved')
    } finally { c.destroy() }
  })

  it('a "Saved" answer covers every earlier check', async () => {
    const room = new Room('')
    const a = client(room)
    try {
      await until(() => a.c.isSynced && a.c.isConfirmed)
      room.holdChecks = true
      for (const word of ['one ', 'two ', 'three ']) {
        a.text.insert(a.text.length, word)
        const before = room.checksReceived
        await until(() => room.checksReceived > before)
      }
      expect(room.held.length).toBeGreaterThanOrEqual(3)
      expect(a.c.status.get().state).toBe('saving')
      room.releaseNewest()
      await until(() => a.c.status.get().state === 'saved')
    } finally { a.c.destroy() }
  })
  it('loads the server copy and never seeds text itself', async () => {
    const room = new Room('# Hello\n')
    const a = client(room)
    await until(() => a.c.isSynced)
    expect(a.text.toString()).toBe('# Hello\n')
    // Reconnecting twice must not duplicate anything.
    a.transport.drop()
    a.c.connect()
    await until(() => a.c.isSynced)
    expect(a.text.toString()).toBe('# Hello\n')
  })

  it('forwards late persistence updates and waits for their receipt', async () => {
    const room = new Room('base')
    const doc = new Y.Doc()
    const transport = memoryTransport(() => room)
    const persistence = Symbol('persistence')
    const c = new SyncClient({ doc, transport, gate: new ImeGate(),
      quietOrigins: [persistence], checkDelayMs: 5,
    })
    try {
      c.connect()
      await until(() => c.isSynced && c.status.get().state === 'saved')
      room.checkDelayMs = 80
      doc.transact(() => doc.getText('md').insert(0, 'cached '), persistence)
      expect(c.status.get().state).toBe('saving')
      await until(() => room.text === 'cached base')
      expect(c.status.get().state).toBe('saving')
      await until(() => c.status.get().state === 'saved')
    } finally { c.destroy() }
  })

  it('two devices typing at once converge without duplicates', async () => {
    const room = new Room('start\n')
    const a = client(room)
    const b = client(room)
    await until(() => a.c.isSynced && b.c.isSynced)
    a.text.insert(0, 'A1 ')
    b.text.insert(b.text.length, 'B1\n')
    a.text.insert(a.text.length, 'A2\n')
    await until(() => a.text.toString() === b.text.toString() && room.text === a.text.toString())
    const t = room.text
    for (const tok of ['A1', 'A2', 'B1', 'start']) expect(t.split(tok).length - 1).toBe(1)
  })

  it('says "saved" only once the server confirms', async () => {
    const room = new Room('')
    room.checkDelayMs = 60
    const a = client(room)
    await until(() => a.c.isSynced)
    await until(() => a.c.status.get().state === 'saved')
    a.text.insert(0, 'hi')
    expect(a.c.status.get().state).toBe('saving')
    await tick(30)
    expect(a.c.status.get().state).toBe('saving')
    await until(() => a.c.status.get().state === 'saved')
    expect(room.text).toBe('hi')
  })

  it('joining snapshots wait for a durable receipt before confirming the replay base', async () => {
    const room = new Room('live snapshot')
    room.holdChecks = true
    let confirmations = 0
    const doc = new Y.Doc()
    const transport = memoryTransport(() => room)
    const c = new SyncClient({ doc, transport, gate: new ImeGate(), checkDelayMs: 5, baseCheckDelayMs: 5,
      onConfirmed: () => { confirmations++ },
    })
    try {
      c.connect()
      await until(() => c.isSynced && room.held.length > 0)
      expect(doc.getText('md').toString()).toBe('live snapshot')
      // The snapshot and Saved use the same receipt authority.
      expect(c.status.get().state).toBe('saving')
      expect(c.isConfirmed).toBe(false)
      expect(confirmations).toBe(0)
      room.releaseChecks()
      await until(() => confirmations > 0)
      expect(c.isConfirmed).toBe(true)
    } finally { c.destroy() }
  })

  it('collaborators\' changes wait for a receipt, with one check per pause, not per change', async () => {
    const room = new Room('start')
    const a = client(room)
    const b = client(room)
    let checksFromA = 0
    const send = a.transport.send.bind(a.transport)
    a.transport.send = (data) => {
      if (decodeSaveCheck(data) != null) checksFromA++
      send(data)
    }
    try {
      await until(() => a.c.isSynced && b.c.isSynced && a.c.isConfirmed && b.c.isConfirmed)
      checksFromA = 0
      room.holdChecks = true
      for (let i = 0; i < 20; i++) b.text.insert(b.text.length, ` w${i}`)
      await until(() => a.text.toString().endsWith('w19'))
      // Our Saved status remains stable; the replay base still waits for their receipt.
      expect(a.c.status.get().state).toBe('saved')
      expect(a.c.isConfirmed).toBe(false)
      await until(() => checksFromA > 0, 3000)
      expect(checksFromA).toBe(1)
      room.releaseChecks()
      await until(() => a.c.isConfirmed)
      expect(checksFromA).toBe(1)
    } finally { a.c.destroy(); b.c.destroy() }
  })

  it('a tab never confirms a sibling tab\'s edit the server has not got (review 3, R3-TAB)', async () => {
    const room = new Room('start')
    const channel = `r3-${Math.random()}`
    const tab = (stall: boolean) => {
      const doc = new Y.Doc()
      const transport = memoryTransport(() => room)
      const send = transport.send.bind(transport)
      // A socket that still says "connected" but whose frames are stuck
      // (a large upload ahead, half-open TCP, a frame the server refused).
      let stuck = false
      transport.send = (data) => { if (!(stall && stuck)) send(data) }
      const seenAtConfirm: boolean[] = []
      const c = new SyncClient({ doc, transport, gate: new ImeGate(), channel, lineage: () => room.lineage,
        checkDelayMs: 5, baseCheckDelayMs: 5, onConfirmed: () => seenAtConfirm.push(room.text.includes('<TAB-B>')) })
      return { c, stick: () => (stuck = true), seenAtConfirm, text: doc.getText('md') }
    }
    const a = tab(false)
    const b = tab(true)
    try {
      a.c.connect(); b.c.connect()
      await until(() => a.c.isSynced && b.c.isSynced && a.c.isConfirmed && b.c.isConfirmed)
      b.stick()
      const before = a.seenAtConfirm.length
      b.text.insert(b.text.length, ' <TAB-B>')
      await until(() => a.text.toString().includes('<TAB-B>'))
      // A holds B's edit as this device's unsaved edit until a receipt covers it.
      expect(a.c.isConfirmed).toBe(false)
      expect(a.c.status.get().state).toBe('saving')
      // B's receipt never comes, so A sends the edit itself and is confirmed by its own receipt.
      await until(() => room.text.includes('<TAB-B>') && a.c.isConfirmed && a.c.status.get().state === 'saved', 5000)
      expect(a.seenAtConfirm.slice(before).every((has) => has)).toBe(true)
    } finally { a.c.destroy(); b.c.destroy() }
  })

  it('tabs share edits without re-sending each other\'s updates', async () => {
    const room = new Room('start')
    const channel = `tabs-${Math.random()}`
    const tab = () => {
      const doc = new Y.Doc()
      const transport = memoryTransport(() => room)
      const sent = { updates: 0 }
      const send = transport.send.bind(transport)
      transport.send = (data) => {
        if (data[0] === 0 && data[1] === 2) sent.updates++
        send(data)
      }
      const c = new SyncClient({ doc, transport, gate: new ImeGate(), channel, lineage: () => room.lineage,
        checkDelayMs: 5, baseCheckDelayMs: 5 })
      return { doc, transport, c, sent, text: doc.getText('md') }
    }
    const a = tab()
    const b = tab()
    try {
      a.c.connect()
      b.c.connect()
      await until(() => a.c.isSynced && b.c.isSynced)
      b.sent.updates = 0
      a.text.insert(a.text.length, ' from a')
      await until(() => b.text.toString() === 'start from a' && room.text === 'start from a')
      await until(() => a.c.status.get().state === 'saved' && b.c.status.get().state === 'saved')
      expect(b.sent.updates).toBe(0)
      // An offline tab's edit is forwarded by a connected one.
      a.transport.drop()
      await until(() => !a.transport.connected)
      a.text.insert(a.text.length, ' offline')
      await until(() => room.text === 'start from a offline')
      expect(b.sent.updates).toBe(1)
    } finally { a.c.destroy(); b.c.destroy() }
  })

  it('automatically replays unsaved browser edits after the server loses its live room', async () => {
    let room = new Room('start')
    const doc = new Y.Doc()
    const transport = memoryTransport(() => room)
    const c = new SyncClient({ doc, transport, gate: new ImeGate(), checkDelayMs: 5 })
    try {
      c.connect()
      await until(() => c.isSynced && c.status.get().state === 'saved')
      const committed = Y.encodeStateAsUpdate(room.doc)
      const lineage = room.lineage
      room.checkDelayMs = 80
      doc.getText('md').insert(5, ' unsaved')
      await until(() => room.text.includes('unsaved'))
      expect(c.status.get().state).toBe('saving')
      transport.drop()
      room = new Room()
      Y.applyUpdate(room.doc, committed)
      room.lineage = lineage
      c.connect()
      await until(() => c.isSynced && c.status.get().state === 'saved')
      expect(room.text).toBe('start unsaved')
      transport.drop()
      c.connect()
      await until(() => c.isSynced && c.status.get().state === 'saved')
      expect(room.text).toBe('start unsaved')
    } finally { c.destroy() }
  })

  it('offline edits are "not saved yet" and reach the server on reconnect', async () => {
    const room = new Room('x')
    const a = client(room)
    await until(() => a.c.isSynced)
    a.transport.drop()
    await tick(5)
    a.text.insert(1, 'yz')
    const s = a.c.status.get()
    expect(s.state).toBe('not-saved')
    expect(typeof s.since).toBe('number')
    expect(room.text).toBe('x')
    a.c.connect()
    await until(() => a.c.status.get().state === 'saved')
    expect(room.text).toBe('xyz')
  })

  it('deletes made offline are kept, not resurrected', async () => {
    const room = new Room('keep remove keep')
    const a = client(room)
    const b = client(room)
    await until(() => a.c.isSynced && b.c.isSynced)
    a.transport.drop()
    await tick(5)
    a.text.delete(5, 7)
    b.text.insert(0, '> ')
    a.c.connect()
    await until(() => a.text.toString() === room.text && b.text.toString() === room.text)
    expect(room.text).toBe('> keep keep')
  })

  it('waits for an open composition before applying remote changes', async () => {
    const room = new Room('abc')
    const gate = new ImeGate()
    const a = client(room, { gate })
    const b = client(room)
    await until(() => a.c.isSynced && b.c.isSynced)
    gate.start()
    b.text.insert(0, 'REMOTE ')
    await until(() => room.text.startsWith('REMOTE'))
    await tick(10)
    expect(a.text.toString()).toBe('abc')
    expect(gate.pending).toBeGreaterThan(0)
    gate.end()
    expect(a.text.toString()).toBe('REMOTE abc')
  })

  it('a client with an old lineage is turned away, not merged', async () => {
    const room = new Room('old text')
    const doc = new Y.Doc()
    let held: string | null = null
    const t: MemoryTransport = memoryTransport(
      () => room,
      () => held,
      (l) => (held ??= l),
    )
    let mismatch = ''
    const c = new SyncClient({ doc, transport: t, gate: new ImeGate(), onLineageMismatch: (l) => (mismatch = l) })
    c.connect()
    await until(() => c.isSynced)
    room.seed('old text')
    await until(() => mismatch !== '')
    expect(mismatch).toBe(room.lineage)
    expect(room.text).toBe('old text')
  })
})

describe('note session', () => {
  it('keeps the safe replay base behind unjournaled remote state', async () => {
    const room = new Room('durable base')
    const meta = memoryMetaStore()
    const s = await NoteSession.open({ id: 'durability-base', offline: false, meta, checkDelayMs: 5,
      transport: () => memoryTransport(() => room),
    })
    const base = async () => (await meta.get('durability-base')).base
    try {
      s.connect()
      let stored: string | null = null
      await until(() => { void base().then((b) => (stored = b)); return stored === 'durable base' })
      room.holdChecks = true
      room.doc.getText('md').insert(0, 'unjournaled ')
      await until(() => s.text.toString().startsWith('unjournaled') && room.held.length > 0, 3000)
      expect(s.status.get().state).toBe('saved')
      expect(s.client.isConfirmed).toBe(false)
      expect(await base()).toBe('durable base')
      room.releaseChecks()
      await until(() => { void base().then((b) => (stored = b)); return stored === 'unjournaled durable base' })
    } finally { s.destroy() }
  })

  it('a collaborator\'s change held unconfirmed makes signing out warn (review 3)', async () => {
    const room = new Room('start\n')
    const meta = memoryMetaStore()
    const id = `held-${Math.random().toString(36).slice(2)}`
    const s = await NoteSession.open({ id, offline: false, meta, checkDelayMs: 5, transport: () => memoryTransport(() => room) })
    const typist = client(room)
    try {
      s.connect()
      await until(() => s.client.isConfirmed && typist.c.isConfirmed)
      expect(pendingNotes()).toBe(false)
      room.holdChecks = true
      typist.text.insert(typist.text.length, 'their words\n')
      await until(() => s.text.toString().includes('their words'))
      // Normal collaborator receipts should not warn while typing continues.
      expect(s.status.get().state).toBe('saved')
      expect(s.client.isConfirmed).toBe(false)
      expect(pendingNotes()).toBe(false)
      expect(pendingNotes(Date.now())).toBe(true)
      let held: number | null | undefined = null
      await until(() => { void meta.get(id).then((m) => (held = m.heldSince)); return held != null }, 5000)
      expect(pendingNotes()).toBe(true)
      room.releaseChecks()
      await until(() => { void meta.get(id).then((m) => (held = m.heldSince)); return held == null && !pendingNotes() }, 5000)
    } finally { s.destroy(); typist.c.destroy() }
  })

  it('closing a note immediately records held changes before its delayed warning marker', async () => {
    const room = new Room('start\n')
    const meta = memoryMetaStore()
    const id = `held-close-${Math.random().toString(36).slice(2)}`
    const s = await NoteSession.open({ id, offline: false, meta, checkDelayMs: 5, transport: () => memoryTransport(() => room) })
    const typist = client(room)
    try {
      s.connect()
      await until(() => s.client.isConfirmed && typist.c.isConfirmed)
      room.holdChecks = true
      typist.text.insert(typist.text.length, 'held on close\n')
      await until(() => s.text.toString().includes('held on close'))
      expect((await meta.get(id)).heldSince ?? null).toBeNull()
      s.destroy()
      expect((await meta.get(id)).heldSince).toBeTypeOf('number')
    } finally { s.destroy(); typist.c.destroy() }
  })

  it('a note closed right after a collaborator change clears its held marker once the receipt arrives', async () => {
    // review4-signout:131 flake (#652): the daily note closed ~150 ms after the
    // server's change arrived; the marker written on close was never cleared,
    // so a sign-out 12 s later warned about a change long since durable.
    const room = new Room('start\n')
    const meta = memoryMetaStore()
    const id = `held-linger-${Math.random().toString(36).slice(2)}`
    const s = await NoteSession.open({ id, offline: false, meta, checkDelayMs: 5, transport: () => memoryTransport(() => room) })
    const typist = client(room)
    try {
      s.connect()
      await until(() => s.client.isConfirmed && typist.c.isConfirmed)
      room.holdChecks = true
      typist.text.insert(typist.text.length, 'arrived just before close\n')
      await until(() => s.text.toString().includes('arrived just before close'))
      s.destroy()
      // Safe at once: if the page went now, the marker keeps this copy.
      expect((await meta.get(id)).heldSince).toBeTypeOf('number')
      room.releaseChecks()
      let held: number | null | undefined = 1
      await until(() => { void meta.get(id).then((m) => (held = m.heldSince)); return held == null }, 5000)
    } finally { s.destroy(); typist.c.destroy() }
  })

  it('a sign-out cutoff waits for fresh held changes but clears after their receipt despite later typing', async () => {
    const room = new Room('start\n')
    const s = await NoteSession.open({ id: `held-cutoff-${Math.random()}`, offline: false,
      meta: memoryMetaStore(), checkDelayMs: 5, transport: () => memoryTransport(() => room),
    })
    try {
      s.connect()
      await until(() => s.client.isConfirmed)
      room.holdChecks = true
      room.doc.getText('md').insert(0, 'first held ')
      await until(() => s.client.heldSince != null)
      const cutoff = Date.now()
      expect(pendingNotes()).toBe(false)
      expect(pendingNotes(cutoff)).toBe(true)
      room.releaseChecks()
      await until(() => s.client.isConfirmed && !pendingNotes(cutoff))
      await tick(5)
      room.holdChecks = true
      room.doc.getText('md').insert(0, 'later held ')
      await until(() => s.client.heldSince != null)
      expect(s.client.heldSince).toBeGreaterThan(cutoff)
      expect(pendingNotes(cutoff)).toBe(false)
      expect(pendingNotes()).toBe(false)
    } finally { s.destroy() }
  })

  it('opening a note offline with no local edits never records it as pending', async () => {
    const id = `offline-open-${Math.random().toString(36).slice(2)}`
    const meta = memoryMetaStore()
    const room = new Room('from the server')
    const open = () => NoteSession.open({ id, offline: true, meta, checkDelayMs: 5,
      transport: () => memoryTransport(() => room) })
    // First visit: synced, nothing typed.
    const first = await open()
    first.connect()
    await until(() => first.client.isSynced)
    first.destroy()
    // Then the server changes and this device opens the note offline.
    room.doc.getText('md').insert(0, 'more ')
    const second = await open()
    try {
      expect(second.status.get().state).toBe('not-saved')
      expect((await meta.get(id)).pendingSince).toBeNull()
    } finally { second.destroy() }
    // A real local edit does mark it, and survives a reopen.
    const third = await open()
    third.text.insert(0, 'mine ')
    let marked: number | null = null
    await until(() => { void meta.get(id).then((m) => (marked = m.pendingSince)); return marked != null })
    third.destroy()
    await meta.put(id, { ...(await meta.get(id)), pendingSince: null })
    const fourth = await open()
    try {
      // The marker kept with the edit in the local copy still says so.
      expect(fourth.status.get().state).toBe('not-saved')
    } finally { fourth.destroy() }
  })

  it('replays only offline changes onto a replaced server copy', async () => {
    const room = new Room('line one\nline two\n')
    const meta = memoryMetaStore()
    let copy: string | null = null
    let transport!: MemoryTransport
    const s = await NoteSession.open({
      id: 'n1',
      offline: false,
      meta,
      checkDelayMs: 5,
      transport: (lineage, learn) => (transport = memoryTransport(() => room, lineage, learn)),
      onConflictCopy: (t) => (copy = t),
    })
    s.connect()
    await until(() => s.status.get().state === 'saved' && s.client.isSynced)
    let base: string | null = null
    await until(() => {
      void meta.get('n1').then((m) => (base = m.base))
      return base === 'line one\nline two\n'
    })
    transport.drop()
    await tick(5)
    s.text.insert(s.text.toString().indexOf('two') + 3, ' and more')
    // Meanwhile the server's copy is re-seeded (e.g. restored) with an extra line.
    room.seed('line zero\nline one\nline two\n')
    s.connect()
    await until(() => room.text.includes('and more'))
    await until(() => s.text.toString() === room.text)
    expect(room.text).toBe('line zero\nline one\nline two and more\n')
    expect(copy).toBeNull()
  })
})

// Regression from the fix-5 authority review: an IME queue can outlive its socket generation.
describe('IME receipts across reconnect', () => {
  it('resends old queued server bytes after a crash before a new connection can confirm them', async () => {
    let room = new Room('durable base')
    const durable = Y.encodeStateAsUpdate(room.doc)
    const lineage = room.lineage
    const meta = memoryMetaStore()
    const id = `ime-restart-${Math.random().toString(36).slice(2)}`
    let transport!: MemoryTransport
    const session = await NoteSession.open({ id, offline: false, meta, checkDelayMs: 5,
      transport: () => (transport = memoryTransport(() => room)),
    })
    let storedBase: string | null = null
    const readBase = () => { void meta.get(id).then((m) => { storedBase = m.base }); return storedBase }
    try {
      session.connect()
      await until(() => session.client.isSynced && session.client.isConfirmed)
      await until(() => readBase() === 'durable base')
      room.holdChecks = true
      session.gate.start()
      // The first server broadcast precedes its failed history transaction. The browser has
      // received these bytes, but its shared document still waits for the composition to end.
      room.doc.getText('md').insert(0, 'UNJOURNALED ')
      await until(() => session.gate.pending > 0)
      expect(session.text.toString()).toBe('durable base')
      transport.drop()
      await tick(5)
      room = new Room()
      Y.applyUpdate(room.doc, durable)
      room.lineage = lineage
      room.holdChecks = true
      session.connect()
      await until(() => room.held.length > 0)
      expect(room.text).toBe('durable base')
      // The reconnect step 2 did not encode the still-queued edit. Its check cannot cover it.
      room.releaseChecks()
      await tick(30)
      expect(session.client.isConfirmed).toBe(false)
      expect(session.status.get().state).not.toBe('saved')
      expect((await meta.get(id)).base).toBe('durable base')
      room.holdChecks = true
      session.gate.end()
      await until(() => room.text.includes('UNJOURNALED'))
      expect(session.client.isConfirmed).toBe(false)
      expect((await meta.get(id)).base).toBe('durable base')
      room.releaseChecks()
      await until(() => session.client.isConfirmed)
      await until(() => readBase() === 'UNJOURNALED durable base')
      expect(room.text).toBe('UNJOURNALED durable base')
      expect(session.text.toString()).toBe(room.text)
    } finally { session.destroy() }
  })

  it('publishes confirmation when a receipted no-op snapshot finally leaves the IME gate', async () => {
    const room = new Room('already cached')
    const doc = new Y.Doc()
    Y.applyUpdate(doc, Y.encodeStateAsUpdate(room.doc))
    const gate = new ImeGate()
    gate.start()
    let confirmations = 0
    const c = new SyncClient({ doc, gate, transport: memoryTransport(() => room),
      checkDelayMs: 5, baseCheckDelayMs: 5, onConfirmed: () => { confirmations++ },
    })
    try {
      c.connect()
      await until(() => c.isConfirmed && gate.pending > 0)
      expect(c.isSynced).toBe(false)
      expect(confirmations).toBe(0)
      gate.end()
      await until(() => c.isSynced)
      expect(confirmations).toBeGreaterThan(0)
      expect(doc.getText('md').toString()).toBe('already cached')
    } finally { c.destroy() }
  })

})


describe('sibling packets held by IME', () => {
  it('observing queued sibling bytes delays Saved before they are displayed', async () => {
    const room = new Room('start')
    const channel = `ime-tab-${Math.random()}`
    const gate = new ImeGate()
    const aDoc = new Y.Doc(), bDoc = new Y.Doc()
    const a = new SyncClient({ doc: aDoc, transport: memoryTransport(() => room), gate, channel,
      lineage: () => room.lineage, checkDelayMs: 5, baseCheckDelayMs: 5 })
    const bTransport = memoryTransport(() => room)
    const send = bTransport.send.bind(bTransport)
    let stuck = false
    bTransport.send = (bytes) => { if (!stuck) send(bytes) }
    const b = new SyncClient({ doc: bDoc, transport: bTransport, gate: new ImeGate(), channel,
      lineage: () => room.lineage, checkDelayMs: 5, baseCheckDelayMs: 5 })
    try {
      a.connect(); b.connect()
      await until(() => a.isConfirmed && b.isConfirmed)
      gate.start(); stuck = true
      bDoc.getText('md').insert(0, 'not on server ')
      await until(() => gate.pending > 0)
      await tick(100)
      expect(aDoc.getText('md').toString()).toBe('start')
      expect(a.status.get().state).toBe('saving')
      expect(a.isConfirmed).toBe(false)
      gate.end()
      await until(() => a.isConfirmed && room.text.includes('not on server'), 5000)
    } finally { a.destroy(); b.destroy() }
  })
})

describe('sibling refusal hints', () => {
  it('a sibling refusal hint needs a fresh full submission and its real receipt to clear', async () => {
    const room = new Room('start')
    const channel = `refusal-${Math.random()}`
    const make = () => {
      const doc = new Y.Doc()
      const c = new SyncClient({ doc, transport: memoryTransport(() => room), gate: new ImeGate(), channel,
        lineage: () => room.lineage, checkDelayMs: 5, baseCheckDelayMs: 5 })
      return { doc, c }
    }
    const a = make(), b = make()
    try {
      a.c.connect(); b.c.connect()
      await until(() => a.c.isConfirmed && b.c.isConfirmed)
      room.holdChecks = true
      b.doc.getText('md').insert(0, 'refused words ')
      await until(() => a.doc.getText('md').toString().includes('refused words'))
      b.c.refuse('error')
      await until(() => a.c.status.get().error === 'Could not save')
      room.releaseChecks()
      await until(() => a.c.isConfirmed && b.c.isConfirmed)
      for (const t of [a, b]) {
        expect(t.c.status.get().state).toBe('saved')
        expect(t.c.status.get().error).toBeUndefined()
      }
    } finally { a.c.destroy(); b.c.destroy() }
  })
})
