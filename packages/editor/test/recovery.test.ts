import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { NoteSession, pendingNotes } from '../src/note'
import { idbMetaStore, memoryMetaStore } from '../src/offline/meta'
import { type MemoryTransport, memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean | Promise<boolean>, ms = 4000) {
  const end = Date.now() + ms
  while (!await f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) }
}

async function fixture(offline = false) {
  const id = `recovery-${crypto.randomUUID()}`
  const meta = offline ? idbMetaStore() : memoryMetaStore()
  const room = new Room('BASE\n')
  let transport!: MemoryTransport
  const open = () => NoteSession.open({ id, offline, meta, checkDelayMs: 5,
    transport: (lineage, learn) => (transport = memoryTransport(() => room, lineage, learn)) })
  const s = await open()
  s.connect()
  await until(async () => s.client.isConfirmed && (await meta.get(id)).base === 'BASE\n')
  transport.drop(); await tick()
  s.text.insert(s.text.length, 'UNSAVED\n')
  room.seed('NEW\nBASE\n')
  return { id, meta, room, s, open }
}

describe('durable recovery obligations', () => {
  for (const held of [true, false]) it(held
    ? 'receipted IME-held text is not replayed twice after a durable reseed'
    : 'fully confirmed text is not replayed twice before the delayed base write', async () => {
    const id = `confirmed-recovery-${crypto.randomUUID()}`
    const meta = memoryMetaStore(), room = new Room('BASE\n')
    let afterMessage = () => {}
    const s = await NoteSession.open({ id, offline: false, meta, checkDelayMs: 5,
      transport: (lineage, learn) => {
        const t = memoryTransport(() => room, lineage, learn)
        const register = t.onMessage.bind(t)
        t.onMessage = (fn) => register((data) => { fn(data); afterMessage() })
        return t
      } })
    let replaced = false
    let baseBeforeReseed: Promise<string | null> | null = null
    s.onReplaced(() => { replaced = true })
    try {
      s.connect()
      await until(async () => s.client.isConfirmed && (await meta.get(id)).base === 'BASE\n')
      if (held) s.gate.start()
      else afterMessage = () => {
        if (baseBeforeReseed || !s.client.isConfirmed || s.text.toString() !== 'REMOTE\nBASE\n') return
        // Reseed after the full-document receipt and before its delayed base write.
        baseBeforeReseed = meta.get(id).then((m) => m.base)
        room.seed('REMOTE\nBASE\n')
      }
      room.doc.getText('md').insert(0, 'REMOTE\n')
      if (held) {
        await until(() => s.client.isConfirmed && s.gate.pending > 0)
        await tick(180)
        baseBeforeReseed = meta.get(id).then((m) => m.base)
        expect(s.status.get().state).toBe('saved')
        room.seed('REMOTE\nBASE\n')
      } else await until(() => baseBeforeReseed != null)
      expect(await baseBeforeReseed).toBe('BASE\n')
      await until(() => replaced && s.client.isConfirmed && !s.readOnly)
      expect(room.text).toBe('REMOTE\nBASE\n')
      expect(s.text.toString()).toBe(room.text)
      expect((await meta.get(id)).base).toBe(room.text)
    } finally { afterMessage = () => {}; s.destroy() }
  })

  it('normal close flushes the receipted IME base before reopen and durable reseed', async () => {
    const id = `closed-confirmed-${crypto.randomUUID()}`
    const meta = idbMetaStore(), room = new Room('BASE\n')
    const options = { id, offline: true, meta, checkDelayMs: 5,
      transport: (lineage: () => string | null, learn: (lineage: string) => void) => memoryTransport(() => room, lineage, learn) }
    const s = await NoteSession.open(options)
    let next: NoteSession | undefined
    try {
      s.connect()
      await until(async () => s.client.isConfirmed && (await meta.get(id)).base === 'BASE\n')
      s.gate.start()
      room.doc.getText('md').insert(0, 'REMOTE\n')
      await until(() => s.client.isConfirmed && s.gate.pending > 0)
      await tick(180)
      expect((await meta.get(id)).base).toBe('BASE\n')
      s.destroy()
      // More server text prevents a local===server replay shortcut from
      // hiding a stale base. Open immediately, without awaiting the flush.
      room.seed('SERVER\nREMOTE\nBASE\n')
      next = await NoteSession.open({ ...options, meta: idbMetaStore() })
      expect((await meta.get(id)).base).toBe('REMOTE\nBASE\n')
      let replaced = false
      next.onReplaced(() => { replaced = true })
      next.connect()
      await until(() => replaced && next!.client.isConfirmed && !next!.readOnly)
      expect(room.text).toBe('SERVER\nREMOTE\nBASE\n')
      expect(next.text.toString()).toBe(room.text)
    } finally { s.destroy(); next?.destroy() }
  })

  it('an IME-held replacement cannot confirm text still held outside its document', async () => {
    const { id, meta, room, s } = await fixture()
    try {
      s.onReplaced(() => s.gate.start())
      s.connect()
      await until(() => s.gate.pending > 0)
      await tick(400)
      expect(room.text).toBe('NEW\nBASE\n')
      expect(s.client.isConfirmed).toBe(false)
      expect(s.status.get().state).not.toBe('saved')
      expect(pendingNotes()).toBe(true)
      expect((await meta.get(id)).base).toBeNull()
      expect((await meta.get(id)).recoverySince).not.toBeNull()
      expect((await meta.recoveries(id)).filter((r) => !r.done)).toHaveLength(1)
      s.gate.end()
      await until(() => room.text.includes('UNSAVED'))
      await until(() => !s.readOnly)
      expect(room.text).toBe('NEW\nBASE\nUNSAVED\n')
    } finally { s.destroy() }
  })

  it('close/reopen restores a held replay and clears it only after its own receipt', async () => {
    const { id, meta, room, s, open } = await fixture(true)
    let next: NoteSession | undefined
    try {
      s.onReplaced(() => s.gate.start())
      s.connect()
      await until(() => s.gate.pending > 0)
      s.destroy()
      next = await open()
      expect(next.readOnly).toBe(true)
      expect(next.client.isConfirmed).toBe(false)
      room.holdChecks = true
      next.connect()
      await until(() => room.text.includes('UNSAVED'))
      expect(next.client.isConfirmed).toBe(false)
      expect((await meta.get(id)).recoverySince).not.toBeNull()
      room.releaseChecks()
      await until(() => !next!.readOnly)
      expect(room.text).toBe('NEW\nBASE\nUNSAVED\n')
      expect((await meta.recoveries(id))[0]?.done).toBe(true)
      expect((await meta.get(id)).recoverySince).toBeNull()
    } finally { s.destroy(); next?.destroy() }
  })

  it('reuses the canonical delta after server apply, before receipt, without inserting twice', async () => {
    const { id, meta, room, s, open } = await fixture(true)
    let next: NoteSession | undefined
    try {
      room.holdChecks = true
      s.connect()
      await until(() => room.text.includes('UNSAVED'))
      const planned = (await meta.recoveries(id))[0]!.plan!.update
      expect(s.client.isConfirmed).toBe(false)
      s.destroy()
      next = await open()
      next.connect()
      await until(() => next!.text.toString().includes('UNSAVED'))
      await tick(40)
      expect((await meta.recoveries(id))[0]!.plan!.update).toEqual(planned)
      expect(room.text).toBe('NEW\nBASE\nUNSAVED\n')
      expect(next.client.isConfirmed).toBe(false)
      room.releaseChecks()
      await until(() => !next!.readOnly)
      expect(room.text.match(/UNSAVED/g)).toHaveLength(1)
    } finally { s.destroy(); next?.destroy() }
  })

  it('ordinary stale metadata cannot erase recovery, and concurrent source captures share one plan', async () => {
    const meta = idbMetaStore()
    const id = crypto.randomUUID()
    const stale = await meta.get(id)
    const source = { lineage: 'old', base: 'BASE', local: 'BASE plus', at: 100 }
    const add = () => meta.updateRecoveries(id, (list) => list.some((r) => r.lineage === source.lineage && r.base === source.base && r.local === source.local)
      ? list : [...list, { ...source, id: crypto.randomUUID() }])
    await Promise.all([add(), add()])
    await meta.put(id, stale)
    const intents = await meta.recoveries(id)
    expect(intents).toHaveLength(1)
    expect((await meta.get(id)).recoverySince).toBe(100)
    const a = new Y.Doc(); a.getText('md').insert(0, 'one')
    const b = new Y.Doc(); b.getText('md').insert(0, 'two')
    const plan = (update: Uint8Array) => meta.updateRecoveries(id, (list) => list.map((r) => r.plan?.lineage === 'new' ? r : { ...r, plan: { lineage: 'new', update, clean: true } }))
    const [first, second] = await Promise.all([plan(Y.encodeStateAsUpdate(a)), plan(Y.encodeStateAsUpdate(b))])
    expect(first[0]!.plan!.update).toEqual(second[0]!.plan!.update)
    a.destroy(); b.destroy()
  })

  it('a canonical replay preserves deletes across reload before its receipt', async () => {
    const { id, meta, room, s, open } = await fixture(true)
    let next: NoteSession | undefined
    try {
      s.text.delete(0, 4)
      room.holdChecks = true
      s.connect()
      await until(() => room.text.includes('UNSAVED'))
      const expected = room.text
      expect(expected).not.toContain('BASE')
      const plan = (await meta.recoveries(id))[0]!.plan!.update
      s.destroy()
      next = await open(); next.connect()
      await until(() => next!.text.toString() === expected)
      expect((await meta.recoveries(id))[0]!.plan!.update).toEqual(plan)
      room.releaseChecks()
      await until(() => !next!.readOnly)
      expect(room.text).toBe(expected)
    } finally { s.destroy(); next?.destroy() }
  })

  it('reopening a target lineage retains held server bytes beside its replay plan', async () => {
    const { id, meta, room, s, open } = await fixture(true)
    let next: NoteSession | undefined
    try {
      room.holdChecks = true
      s.connect()
      await until(() => room.text.includes('UNSAVED'))
      const durable = Y.encodeStateAsUpdate(room.doc)
      const lineage = room.lineage
      room.doc.getText('md').insert(0, 'HELD ')
      await until(() => s.text.toString().startsWith('HELD '))
      s.destroy()
      // The server crashed before journaling HELD. Reopen must retain the
      // target WAL and resubmit it, rather than clearing it as an old copy.
      room.seed('')
      Y.applyUpdate(room.doc, durable)
      room.lineage = lineage
      room.holdChecks = true
      next = await open()
      next.connect()
      await until(() => room.text.startsWith('HELD '))
      expect(next.client.isConfirmed).toBe(false)
      expect((await meta.get(id)).recoverySince).not.toBeNull()
      room.releaseChecks()
      await until(() => !next!.readOnly)
      expect(room.text).toBe('HELD NEW\nBASE\nUNSAVED\n')
    } finally { s.destroy(); next?.destroy() }
  })

  it('an offered conflict copy remains durable and unconfirmed across reload', async () => {
    const id = crypto.randomUUID(), meta = memoryMetaStore(), room = new Room('replacement')
    await meta.updateRecoveries(id, () => [{ id: 'copy', at: 1, lineage: 'old', base: null, local: 'irreplaceable' }])
    let offered = ''
    const open = () => NoteSession.open({ id, offline: false, meta, checkDelayMs: 5,
      onConflictCopy: (text) => { offered = text }, transport: (lineage, learn) => memoryTransport(() => room, lineage, learn) })
    const first = await open()
    first.connect()
    await until(() => offered === 'irreplaceable')
    await tick(350)
    expect(first.client.isConfirmed).toBe(false)
    first.destroy()
    const next = await open()
    try {
      next.connect(); await until(() => next.client.isSynced); await tick(350)
      expect(next.status.get().state).not.toBe('saved')
      expect((await meta.recoveries(id))[0]!.local).toBe('irreplaceable')
      expect((await meta.get(id)).recoverySince).toBe(1)
    } finally { next.destroy() }
  })

  it('a fresh receipted recovery clears its old refusal across reopening', async () => {
    const { id, meta, room, s, open } = await fixture()
    let next: NoteSession | undefined
    try {
      s.client.refuse('too_large')
      s.connect()
      await until(() => room.text.includes('UNSAVED'))
      await until(() => s.client.isConfirmed && !s.readOnly)
      expect(s.status.get().tooLarge).toBe(false)
      expect((await meta.recoveries(id))[0]?.done).toBe(true)
      s.destroy()
      next = await open(); next.connect()
      await until(() => next!.client.isConfirmed)
      expect(next.status.get().tooLarge).toBe(false)
      expect((await meta.get(id)).refused).toBeNull()
    } finally { s.destroy(); next?.destroy() }
  })

  it('capacity failure refuses recovery before clearing the original copy', async () => {
    const { id, meta, s } = await fixture()
    try {
      await meta.updateRecoveries(id, () => Array.from({ length: 32 }, (_, n) => ({
        id: `done-${n}`, at: n, lineage: `old-${n}`, base: '', local: '', done: true,
      })))
      const original = s.doc
      s.connect()
      await until(() => s.status.get().error === 'Could not save')
      expect(s.doc).toBe(original)
      expect(s.text.toString()).toBe('BASE\nUNSAVED\n')
      expect((await meta.recoveries(id))).toHaveLength(32)
      expect(s.client.isConfirmed).toBe(false)
    } finally { s.destroy() }
  })
})
