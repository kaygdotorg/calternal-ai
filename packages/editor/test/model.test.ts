// Independent round-6 model (#652): REAL SyncClient + real Yjs against a server
// model that separates volatile state from durable history.
//
// Independent byte oracles (no receipt-ledger access): Saved must cover every
// device edit observed by the tab and its first server snapshot. onConfirmed
// additionally requires the whole observed document, including volatile peer
// changes, to be durable. Peer traffic must not prevent Saved for our edits.
// Liveness oracle: with no genuine size refusal, once typing stops, every tab
// is online, IME is released and the server commits, every tab reaches Saved.
//
// Covers: 1-3 sibling tabs (BroadcastChannel), offline/online, IME holds,
// too-large refusals, transient join failures that later succeed, tab
// close/reopen carrying pending state, server crash/restart losing
// non-durable state, reordered commit timing. The server follows the real
// room's gap rule (test-server/resync.ts): a change with a gap is refused, a
// resend is asked for and save checks wait for a sync step 2 that fills it.
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest'
import * as Y from 'yjs'
import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import * as syncProtocol from 'y-protocols/sync'
import { SyncClient, TAB_ORIGIN } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { MSG_REFUSED, MSG_SYNC, decodeMessage, decodeSaveCheck, encodeSaved, encodeUpdate } from '../src/sync/protocol'
import { followsOn, hasMissing } from '../test-server/resync'

// This module runs in Vitest's Node process; keep its env type local to tests.
declare const process: { env: Record<string, string | undefined> }
const { env } = process
const TRIALS = Number(env.R6_TRIALS ?? 200)
const SEED0 = Number(env.R6_SEED ?? 1)
const STEPS = Number(env.R6_STEPS ?? 40)
const START = Number(env.R6_START ?? 0)
const TRACE = env.R6_TRACE === '1'
const { LIMIT } = vi.hoisted(() => ({ LIMIT: 1500 }))
// Exercise the actual client refusal and wire-compaction paths, at the same
// framed-message limit as the independent server.
vi.mock('../src/sync/protocol', async (original) => ({
  ...await original<typeof import('../src/sync/protocol')>(), MAX_ROOM_MESSAGE: LIMIT,
}))

// ---- BroadcastChannel stand-in: in-process, async, structured-cloned. ----
const channels = new Map<string, Set<FakeBC>>()
let observeChannel: ((data: unknown) => void) | undefined
class FakeBC {
  onmessage: ((ev: { data: unknown }) => void) | null = null
  closed = false
  private readonly observe = observeChannel
  constructor(readonly name: string) { (channels.get(name) ?? channels.set(name, new Set()).get(name)!).add(this) }
  postMessage(data: unknown) {
    if (this.closed) return
    for (const other of channels.get(this.name) ?? []) if (other !== this) {
      const copy = structuredClone(data)
      queueMicrotask(() => { if (!other.closed) { other.observe?.(copy); other.onmessage?.({ data: copy }) } })
    }
  }
  close() { this.closed = true; channels.get(this.name)?.delete(this) }
}

function errorFrame(tag: number, code: string): Uint8Array {
  const e = encoding.createEncoder()
  encoding.writeVarUint(e, tag)
  encoding.writeVarUint8Array(e, new TextEncoder().encode(JSON.stringify(code)))
  return encoding.toUint8Array(e)
}

type Conn = { alive: boolean; deliver: (d: Uint8Array) => void; kill: (code: number) => void; resync?: boolean; held?: number | null }

class Server {
  doc = new Y.Doc()
  log: Uint8Array[] = []
  durable = 0
  durableDoc = new Y.Doc()
  conns = new Set<Conn>()
  checks: Array<{ conn: Conn; id: number; at: number }> = []
  transient: () => boolean = () => false
  constructor() { this.attach() }
  private attach() {
    this.doc.on('update', (u: Uint8Array, origin: unknown) => {
      this.log.push(u)
      const msg = encodeUpdate(u)
      for (const c of this.conns) if (c !== origin) c.deliver(msg)
    })
  }
  join(conn: Conn) {
    this.conns.add(conn)
    const s1 = encoding.createEncoder(); encoding.writeVarUint(s1, MSG_SYNC); syncProtocol.writeSyncStep1(s1, this.doc); conn.deliver(encoding.toUint8Array(s1))
    const s2 = encoding.createEncoder(); encoding.writeVarUint(s2, MSG_SYNC); syncProtocol.writeSyncStep2(s2, this.doc); conn.deliver(encoding.toUint8Array(s2))
  }
  receive(conn: Conn, data: Uint8Array) {
    if (!this.conns.has(conn)) return
    if (this.transient()) {
      // A temporary room failure loses membership and the unaccepted packet.
      // custom103 is retryable; custom102 is reserved for genuine too_large.
      conn.deliver(errorFrame(103, 'room_unavailable'))
      this.conns.delete(conn); conn.kill(1011)
      return
    }
    const check = decodeSaveCheck(data)
    if (check != null) {
      // While a resend is owed, the newest check waits for it; queued ones are dropped (acks.rs).
      if (conn.resync) { conn.held = check; this.checks = this.checks.filter((c) => c.conn !== conn); return }
      this.checks.push({ conn, id: check, at: this.log.length })
      return
    }
    const d = decoding.createDecoder(data)
    if (decoding.readVarUint(d) !== MSG_SYNC) return
    // Peek payload size; refuse like the real too_large path (refuse + close membership).
    const peek = decoding.createDecoder(data); decoding.readVarUint(peek); const sub = decoding.readVarUint(peek)
    if (sub !== syncProtocol.messageYjsSyncStep1 && data.length > LIMIT) {
      const e = encoding.createEncoder(); encoding.writeVarUint(e, MSG_REFUSED); encoding.writeVarUint8Array(e, new TextEncoder().encode('"too_large"'))
      conn.deliver(encoding.toUint8Array(e))
      this.conns.delete(conn); conn.kill(4400)
      return
    }
    decoding.readVarUint(d)
    if (sub === syncProtocol.messageYjsSyncStep1) {
      const e = encoding.createEncoder(); encoding.writeVarUint(e, MSG_SYNC)
      syncProtocol.writeSyncStep2(e, this.doc, decoding.readVarUint8Array(d))
      conn.deliver(encoding.toUint8Array(e))
      return
    }
    if (sub !== syncProtocol.messageYjsSyncStep2 && sub !== syncProtocol.messageYjsUpdate) return
    const update = decoding.readVarUint8Array(d)
    // The real room's gap rule (crates/notes room/sync.rs; test-server/resync.ts):
    // a change that skips ahead is not applied, a resend is asked for, and
    // only a sync step 2 that fills every gap releases the held check.
    if (!followsOn(this.doc, update)) { this.resync(conn); return }
    const logged = this.log.length
    Y.applyUpdate(this.doc, update, conn)
    // Like the real room (crates/notes pipeline.rs apply_typed), a parked
    // input that changes nothing yet is journaled too: a later input may
    // release it without containing it.
    if (this.log.length === logged && hasMissing(this.doc)) this.log.push(update)
    if (hasMissing(this.doc)) this.resync(conn)
    else if (sub === syncProtocol.messageYjsSyncStep2) this.resynced(conn)
  }
  private resync(conn: Conn) {
    if (conn.resync) return
    conn.resync = true
    const e = encoding.createEncoder(); encoding.writeVarUint(e, MSG_SYNC); syncProtocol.writeSyncStep1(e, this.doc)
    conn.deliver(encoding.toUint8Array(e))
  }
  private resynced(conn: Conn) {
    conn.resync = false
    if (conn.held != null) { this.checks.push({ conn, id: conn.held, at: this.log.length }); conn.held = null }
  }
  commit(n: number) {
    const to = Math.min(this.log.length, this.durable + n)
    for (let i = this.durable; i < to; i++) Y.applyUpdate(this.durableDoc, this.log[i]!)
    this.durable = to
    const ready = this.checks.filter((c) => c.at <= this.durable)
    this.checks = this.checks.filter((c) => c.at > this.durable)
    for (const c of ready) if (this.conns.has(c.conn)) c.conn.deliver(encodeSaved(c.id))
  }
  crash() {
    for (const c of [...this.conns]) c.kill(1006)
    this.conns.clear(); this.checks = []
    this.log = this.log.slice(0, this.durable)
    this.doc.destroy(); this.doc = new Y.Doc(); this.attach()
    const replay = Y.encodeStateAsUpdate(this.durableDoc)
    // Rebuilding from durable history is not a new change.
    const saved = this.log.slice(); Y.applyUpdate(this.doc, replay); this.log = saved
  }
}

type Transport = ReturnType<typeof transport>
function transport(server: () => Server, flaky: () => boolean, observe: (data: Uint8Array) => void = () => {}, sent: (data: Uint8Array) => void = () => {}) {
  let conn: Conn | null = null
  let wanted = false
  let dead = false
  let network = true
  let retry: ReturnType<typeof setTimeout> | null = null
  const opens: Array<() => void> = [], closes: Array<(c: number, r: string) => void> = [], messages: Array<(d: Uint8Array) => void> = [], errors: Array<(c: string) => void> = []
  const close = (code: number) => { if (!conn) return; conn.alive = false; conn = null; queueMicrotask(() => closes.forEach((f) => f(code, ''))) }
  const open = () => {
    if (!wanted || conn || dead || !network) return
    if (flaky()) {
      // Like LiveRoom join_failed with a retryable code: report, then retry.
      queueMicrotask(() => errors.forEach((f) => f('too_many_requests')))
      retry = setTimeout(() => { retry = null; open() }, 1000)
      return
    }
    const c: Conn = {
      alive: true,
      deliver: (d) => queueMicrotask(() => c.alive && (observe(d), messages.forEach((f) => f(d)))),
      kill: (code) => queueMicrotask(() => { if (conn === c) { if (code === 4400) { dead = true; wanted = false; errors.forEach((f) => f('too_large')) } close(code); if (code !== 4400 && wanted) retry = setTimeout(() => { retry = null; open() }, 100) } }),
    }
    conn = c
    queueMicrotask(() => { if (!c.alive) return; opens.forEach((f) => f()); server().join(c) })
  }
  return {
    get connected() { return conn?.alive ?? false },
    connect() { dead = false; wanted = true; open() },
    setOnline(online: boolean) {
      network = online
      if (online) { open(); return }
      if (retry) clearTimeout(retry)
      retry = null
      if (conn) server().conns.delete(conn)
      close(1006)
    },
    disconnect() { wanted = false; if (retry) clearTimeout(retry); retry = null; if (conn) server().conns.delete(conn); close(1000) },
    send(d: Uint8Array) { sent(d); const c = conn; if (c?.alive) queueMicrotask(() => c.alive && server().receive(c, d)) },
    onOpen: (f: () => void) => void opens.push(f),
    onClose: (f: (c: number, r: string) => void) => void closes.push(f),
    onMessage: (f: (d: Uint8Array) => void) => void messages.push(f),
    onError: (f: (c: string) => void) => void errors.push(f),
  }
}

type Tab = { undo: Y.UndoManager; paused: boolean; owned: Uint8Array[]; doc: Y.Doc; client: SyncClient; gate: ImeGate; t: Transport; online: boolean; refusal: 'too_large' | 'error' | null; pending: number | null; held: number | null; unsub: () => void }

describe('round-6 independent SyncClient model', () => {
  beforeAll(() => {
    vi.stubGlobal('BroadcastChannel', FakeBC)
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'] })
    // Every receipt-drain step runs out of time budget at once, so each one
    // yields (a real MessageChannel task) and interleaves with everything else.
    // A race once hid here, seen only on a loaded host.
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => (now += 5))
  })
  afterAll(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks() })

  it('signing out during join backoff cancels retries and retains the pending edit', async () => {
    const server = new Server()
    const doc = new Y.Doc()
    doc.getText('md').insert(0, 'pending before sign-out')
    let joins = 0
    const t = transport(() => server, () => { joins++; return true })
    const client = new SyncClient({ doc, transport: t as any, gate: new ImeGate(), pendingSince: 123 })
    try {
      client.connect()
      await vi.advanceTimersByTimeAsync(50)
      const attempted = joins
      client.disconnect()
      await vi.advanceTimersByTimeAsync(30000)
      expect(joins).toBe(attempted)
      expect(client.status.get().state).not.toBe('saved')
      expect(client.pendingSince).toBe(123)
      expect(doc.getText('md').toString()).toBe('pending before sign-out')
    } finally { client.destroy(); doc.destroy(); server.doc.destroy(); server.durableDoc.destroy() }
  })

  it('waits for a durable initial snapshot even without local typing', async () => {
    const server = new Server()
    server.doc.getText('md').insert(0, 'volatile initial snapshot')
    const doc = new Y.Doc()
    const gate = new ImeGate()
    const t = transport(() => server, () => false)
    const confirmed = vi.fn()
    const client = new SyncClient({ doc, gate, transport: t as any, onConfirmed: confirmed })
    try {
      client.connect()
      await vi.advanceTimersByTimeAsync(1000)
      expect(doc.getText('md').toString()).toBe('volatile initial snapshot')
      expect(client.status.get().state).not.toBe('saved')
      expect(confirmed).not.toHaveBeenCalled()
      server.commit(1000)
      await vi.advanceTimersByTimeAsync(1000)
      expect(client.status.get().state).toBe('saved')
      expect(confirmed).toHaveBeenCalled()
    } finally { client.destroy(); doc.destroy(); server.doc.destroy(); server.durableDoc.destroy() }
  })

  it('returns to Saved after retryable join and custom103 room failures stop', async () => {
    const server = new Server()
    let joinFails = true
    let roomFailures = 2
    server.transient = () => roomFailures > 0 && (roomFailures--, true)
    const doc = new Y.Doc()
    const gate = new ImeGate()
    const t = transport(() => server, () => joinFails)
    const client = new SyncClient({ doc, gate, transport: t as any })
    try {
      client.connect()
      doc.getText('md').insert(0, 'typing during a retryable outage')
      await vi.advanceTimersByTimeAsync(500)
      expect(client.status.get().state).not.toBe('saved')
      expect(client.status.get().tooLarge).toBe(false)
      joinFails = false
      for (let i = 0; i < 40; i++) {
        server.commit(1000)
        await vi.advanceTimersByTimeAsync(250)
      }
      expect(roomFailures).toBe(0)
      expect(Y.snapshotContainsUpdate(Y.snapshot(server.durableDoc), Y.encodeStateAsUpdate(doc))).toBe(true)
      expect(client.status.get().state).toBe('saved')
      expect(client.status.get().error).toBeUndefined()
      expect(client.isConfirmed).toBe(true)
    } finally { client.destroy(); doc.destroy(); server.doc.destroy(); server.durableDoc.destroy() }
  })

  it('confirms the final small state after an IME-held rejected paste and deletion cross a reconnect', async () => {
    const server = new Server()
    server.doc.getText('md').insert(0, 'seed')
    server.commit(1000)
    const docs = [new Y.Doc(), new Y.Doc()]
    const gates = [new ImeGate(), new ImeGate()]
    const transports = docs.map(() => transport(() => server, () => false))
    const clients = docs.map((doc, i) => new SyncClient({ doc, gate: gates[i]!, transport: transports[i]! as any, channel: 'held-paste-reconnect' }))
    try {
      clients.forEach((client) => client.connect())
      await vi.advanceTimersByTimeAsync(500)
      server.commit(1000)
      await vi.advanceTimersByTimeAsync(500)
      expect(clients.every((client) => client.status.get().state === 'saved')).toBe(true)
      gates[1]!.start()
      docs[0]!.getText('md').insert(0, 'P'.repeat(LIMIT + 50))
      await vi.advanceTimersByTimeAsync(0)
      expect(clients[0]!.status.get().tooLarge).toBe(true)
      expect(gates[1]!.pending).toBeGreaterThan(0)
      transports[1]!.setOnline(false)
      docs[0]!.getText('md').delete(0, docs[0]!.getText('md').length)
      for (let i = 0; i < 8; i++) { await vi.advanceTimersByTimeAsync(250); server.commit(1000) }
      transports[1]!.setOnline(true)
      await vi.advanceTimersByTimeAsync(500)
      expect(gates[1]!.composing).toBe(true)
      expect(clients[1]!.isConfirmed).toBe(false)
      gates[1]!.end()
      for (let i = 0; i < 40; i++) { await vi.advanceTimersByTimeAsync(250); server.commit(1000) }
      await vi.advanceTimersByTimeAsync(0)
      for (let i = 0; i < clients.length; i++) {
        expect(docs[i]!.getText('md').toString()).toBe('')
        expect(Y.encodeStateAsUpdate(docs[i]!).length).toBeLessThan(LIMIT)
        expect(Y.snapshotContainsUpdate(Y.snapshot(server.durableDoc), Y.encodeStateAsUpdate(docs[i]!))).toBe(true)
        expect(clients[i]!.status.get().state).toBe('saved')
        expect(clients[i]!.isConfirmed).toBe(true)
      }
    } finally {
      clients.forEach((client) => client.destroy())
      docs.forEach((doc) => doc.destroy())
      server.doc.destroy(); server.durableDoc.destroy()
      channels.clear(); vi.clearAllTimers()
    }
  })

  it('saves our covered edit while a later peer update still blocks replay-base confirmation', async () => {
    const server = new Server()
    server.doc.getText('md').insert(0, 'seed')
    server.commit(1000)
    const doc = new Y.Doc()
    const gate = new ImeGate()
    const t = transport(() => server, () => false)
    const confirmed = vi.fn()
    const client = new SyncClient({ doc, gate, transport: t as any, onConfirmed: confirmed })
    try {
      client.connect()
      await vi.advanceTimersByTimeAsync(1000)
      server.commit(1000)
      await vi.advanceTimersByTimeAsync(1000)
      expect(client.status.get().state).toBe('saved')
      confirmed.mockClear()
      doc.getText('md').insert(doc.getText('md').length, ' our edit')
      await vi.advanceTimersByTimeAsync(200) // local save check is now on the server
      const throughOurEdit = server.log.length
      server.doc.getText('md').insert(0, 'volatile peer ')
      await vi.advanceTimersByTimeAsync(0) // observe peer before our receipt
      server.commit(throughOurEdit - server.durable)
      await vi.advanceTimersByTimeAsync(0)
      expect(Y.snapshotContainsUpdate(Y.snapshot(server.durableDoc), Y.encodeStateAsUpdate(doc))).toBe(false)
      expect(client.status.get().state).toBe('saved')
      expect(client.isConfirmed).toBe(false)
      expect(confirmed).not.toHaveBeenCalled()
      await vi.advanceTimersByTimeAsync(1000)
      server.commit(1000)
      await vi.advanceTimersByTimeAsync(1000)
      expect(client.isConfirmed).toBe(true)
      expect(confirmed).toHaveBeenCalled()
    } finally { client.destroy(); doc.destroy(); server.doc.destroy(); server.durableDoc.destroy() }
  })

  it(`${TRIALS} trials keep device Saved and whole-document onConfirmed behind durable history, and Saved returns`, async () => {
    const stats = { seed: SEED0, start: START, trials: TRIALS, roomErrors: 0, saved: 0, confirmed: 0, refusedTrials: 0, transient: 0, crashes: 0, liveness: 0, missingCoverage: 0, deliveryFailures: [] as string[], fullRetries: 0, retryBytes: 0, undo: 0, redo: 0, signouts: 0, byteBudgetFailures: [] as string[], fitsButNotSaved: [] as string[], coveredButUnsaved: 0, livenessFailures: [] as string[] }
    for (let trial = START; trial < START + TRIALS; trial++) {
      let seed = (SEED0 * 1_000_003 + trial * 7919) | 0 || 1
      const random = (n: number) => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return (seed >>> 0) % n }
      const server = new Server()
      server.doc.clientID = 1
      let nextClient = 100
      const trace: string[] = []
      server.doc.getText('md').insert(0, 'seed\n'); server.commit(1)
      const channel = `note-${trial}`
      let flakyOn = env.R6_FLAKY !== '0'
      server.transient = () => { const fail = flakyOn && random(35) === 0; if (fail) stats.roomErrors++; return fail }
      let genuineRefusal = false
      let transientSeen = false
      const violation = (tab: Tab, what: string) => {
        const snapshot = Y.snapshot(server.durableDoc)
        const updates = what === 'onConfirmed' ? [Y.encodeStateAsUpdate(tab.doc)] : tab.owned
        const ok = updates.every((update) => Y.snapshotContainsUpdate(snapshot, update))
        if (!ok) throw new Error(`${what} without durable coverage: trial ${trial} seed ${SEED0}; trace=${trace.join(' | ')}`)
      }
      const open = (carry?: { owned: Uint8Array[]; state: Uint8Array; pending: number | null; held: number | null; refusal: 'too_large' | 'error' | null }): Tab => {
        const doc = new Y.Doc()
        doc.clientID = nextClient++
        if (carry) Y.applyUpdate(doc, carry.state, 'local-copy')
        const gate = new ImeGate()
        const owned = carry?.owned.slice() ?? []
        const undo = new Y.UndoManager(doc.getText('md'))
        let deletions = 0, refusals = 0, connections = 0
        const fullRetries = new Map<string, { copies: number; bytes: number; largest: number }>()
        // Packets this tab received and may pass on unchanged (a held packet
        // resent on its own): relaying them is not a new full-state copy.
        const relayed = new Set<string>()
        const key = (u: Uint8Array) => u.join(',')
        // Resend requests (the room's sync step 1, at join or after a gap) per connection:
        // each one is owed a step 2, which may be a complete copy.
        const asked = new Map<number, number>()
        doc.on('afterTransaction', (tr: Y.Transaction) => { if (tr.deleteSet.clients.size) deletions++ })
        let initialSnapshot = true
        const t = transport(() => server, () => { const f = flakyOn && random(6) === 0; if (f) transientSeen = true; return f }, (data) => {
          const msg = decodeMessage(data)
          if (msg.kind === 'refused') refusals++
          if (msg.kind === 'update' || msg.kind === 'step2') relayed.add(key(msg.update))
          if (msg.kind === 'step1') asked.set(connections, (asked.get(connections) ?? 0) + 1)
          const d = decoding.createDecoder(data)
          if (decoding.readVarUint(d) !== MSG_SYNC) return
          if (decoding.readVarUint(d) === syncProtocol.messageYjsSyncStep2 && initialSnapshot) {
            initialSnapshot = false
            owned.push(decoding.readVarUint8Array(d))
          }
        }, (data) => {
          const msg = decodeMessage(data)
          if (msg.kind === 'step1') connections++
          if ((msg.kind !== 'update' && msg.kind !== 'step2') || !tab.refusal || relayed.has(key(msg.update))) return
          const vector = Y.decodeStateVector(Y.encodeStateVectorFromUpdate(msg.update))
          const current = Y.decodeStateVector(Y.encodeStateVector(doc))
          if (!current.size || ![...current].every(([id, clock]) => (vector.get(id) ?? 0) >= clock)) return
          const epoch = `${connections}/${deletions}`
          const budget = fullRetries.get(epoch) ?? { copies: 0, bytes: 0, largest: 0 }
          budget.copies++; budget.bytes += data.length; budget.largest = Math.max(budget.largest, data.length)
          // New oversized packets and refusal hints may independently refuse
          // a retry already in flight. Each grants one copy, never one per key.
          const allowed = budget.largest * (refusals + (asked.get(connections) ?? 0) + 1)
          if (budget.bytes > allowed) stats.byteBudgetFailures.push(`trial ${trial}: ${budget.bytes} full-state bytes exceed ${allowed} in ${epoch}`)
          fullRetries.set(epoch, budget)
          stats.fullRetries++; stats.retryBytes += data.length
        })
        const tab = {} as Tab
        Object.assign(tab, { undo, paused: false, owned, doc, gate, t, online: true, refusal: carry?.refusal ?? null, pending: carry?.pending ?? null, held: carry?.held ?? null })
        // Register before SyncClient, so synchronous status publication cannot
        // precede the oracle's observation of a local edit.
        doc.on('update', (update: Uint8Array, origin: unknown) => {
          if (origin !== tab.client && origin !== TAB_ORIGIN) owned.push(update)
          if (encodeUpdate(update).length > LIMIT) refusals++
        })
        observeChannel = (value) => {
          const msg = value as { t?: string; u?: Uint8Array }
          // The generated sibling stream contains device edits (and the
          // already-durable seed); observe before the client's IME gate.
          if (msg.t === 'u' && msg.u instanceof Uint8Array) { owned.push(msg.u); relayed.add(key(msg.u)) }
          if (msg.t === 'no' || (msg.u instanceof Uint8Array && encodeUpdate(msg.u).length > LIMIT)) refusals++
        }
        tab.client = new SyncClient({
          doc, transport: t as any, gate, channel, checkDelayMs: 150, baseCheckDelayMs: 300,
          pendingSince: carry?.pending ?? null, heldSince: carry?.held ?? null, refused: carry?.refusal ?? null,
          onPendingChange: (s) => { tab.pending = s }, onHeldChange: (s) => { tab.held = s },
          onRefusalChange: (c) => { tab.refusal = c },
          onConfirmed: () => { stats.confirmed++; violation(tab, 'onConfirmed') },
        })
        observeChannel = undefined
        tab.unsub = tab.client.status.subscribe((s) => { if (s.state === 'saved') { stats.saved++; violation(tab, 'Saved') } })
        tab.client.connect()
        return tab
      }
      const tabs: Tab[] = Array.from({ length: 1 + random(3) }, () => open())
      for (let step = 0; step < STEPS; step++) {
        const tab = tabs[random(tabs.length)]!
        const text = tab.doc.getText('md')
        const action = random(16)
        trace.push(`${step}:${tabs.indexOf(tab)}:${action}`)
        if (trace.length > 40) trace.shift()
        switch (action) {
          case 0: case 1: case 2: case 3: text.insert(random(text.length + 1), 'abcdefgh'.slice(0, 1 + random(6))); tab.undo.stopCapturing(); break
          case 4: if (text.length > 2) text.delete(random(text.length - 1), 1 + random(2)); break
          case 5: if (random(10) === 0) { text.insert(random(text.length + 1), 'P'.repeat(LIMIT + 50)) } break
          case 6: tab.online = !tab.online; tab.t.setOnline(tab.online); break
          case 7: case 8: server.commit(1 + random(4)); break
          case 9: if (random(3) === 0) { server.crash(); stats.crashes++ } break
          case 10: if (tab.gate.composing) tab.gate.end(); else tab.gate.start(); break
          case 11: { // close and reopen this tab from its final local copy
            // destroy() releases the IME gate; snapshot that final state.
            tab.gate.end()
            const carry = { owned: tab.owned, state: Y.encodeStateAsUpdate(tab.doc), pending: tab.client.pendingSince, held: tab.client.heldSince, refusal: tab.refusal }
            tab.unsub(); tab.client.destroy(); tab.undo.destroy()
            tabs[tabs.indexOf(tab)] = open(carry)
            break
          }
          case 12: if (text.length > LIMIT) text.delete(0, text.length); break
          case 13: tab.undo.undo(); stats.undo++; break
          case 14: tab.undo.redo(); stats.redo++; break
          case 15:
            // Sign-out pauses sync even when a retry is in backoff. A later
            // cancellation may resume it without losing the pending copy.
            tab.paused = !tab.paused
            if (tab.paused) { tab.client.disconnect(); stats.signouts++ }
            else tab.client.connect()
            break
          default: break
        }
        await vi.advanceTimersByTimeAsync(random(400))
        if (TRACE) console.log('R6-TRACE', trial, step, tabs.map((t) => ({ client: t.doc.clientID, status: t.client.status.get(), online: t.online, conn: t.t.connected, ime: t.gate.composing, bytes: Y.encodeStateAsUpdate(t.doc).length, text: t.doc.getText('md').length, refusal: t.refusal })))
        for (const t of tabs) if (t.client.status.get().state === 'saved') violation(t, 'Saved (poll)')
      }
      // Quiesce: no more typing, no flaky joins, IME released, everyone online, server commits.
      flakyOn = false
      for (const t of tabs) { if (t.paused) { t.paused = false; t.client.connect() }; if (t.gate.composing) t.gate.end(); if (!t.online) { t.t.setOnline(true); t.online = true } }
      for (let i = 0; i < 40; i++) { server.commit(1000); await vi.advanceTimersByTimeAsync(250) }
      if (TRACE) console.log('R6-TRACE-QUIET', trial, tabs.map((t) => ({ client: t.doc.clientID, status: t.client.status.get(), conn: t.t.connected, bytes: Y.encodeStateAsUpdate(t.doc).length, refusal: t.refusal })))
      genuineRefusal = tabs.some((t) => t.refusal === 'too_large')
      if (genuineRefusal) stats.refusedTrials++
      if (transientSeen) stats.transient++
      const covered = tabs.every((t) => t.owned.every((u) => Y.snapshotContainsUpdate(Y.snapshot(server.durableDoc), u)))
      if (!genuineRefusal && !covered) {
        stats.missingCoverage++
        if (stats.deliveryFailures.length < 5) stats.deliveryFailures.push(`trial ${trial} seed ${SEED0}; trace=${trace.join(' | ')}`)
      }
      if (covered) {
        stats.liveness++
        const stuck = tabs.filter((t) => t.client.status.get().state !== 'saved')
        if (stuck.length && stats.livenessFailures.length < 5) stats.livenessFailures.push(`trial ${trial}: ${stuck.map((t) => `${t.client.status.get().state}/${t.client.status.get().error ?? ''}/refusal=${t.refusal}/conn=${t.t.connected}/synced=${t.client.isSynced}/pend=${t.client.pendingSince}/held=${t.client.heldSince}/bytes=${Y.encodeStateAsUpdate(t.doc).length}/text=${t.doc.getText('md').length}`).join(', ')} transient=${transientSeen}; trace=${trace.join(' | ')}`)
        if (stuck.length) stats.coveredButUnsaved++
        if (stuck.length) (stats as any).stuckTrials = ((stats as any).stuckTrials ?? 0) + 1
      }
      // Liveness after a refusal: once faults stop, a tab whose garbage-
      // collected state fits the limit must have reached Saved again.
      for (const t of tabs) {
        const wire = new Y.Doc(); Y.applyUpdate(wire, Y.encodeStateAsUpdate(t.doc))
        const fits = encodeUpdate(Y.encodeStateAsUpdate(wire)).length <= LIMIT
        wire.destroy()
        const st = t.client.status.get()
        if (fits && st.state !== 'saved' && stats.fitsButNotSaved.length < 5) stats.fitsButNotSaved.push(`trial ${trial}: ${st.state}/tooLarge=${!!st.tooLarge}/${st.error ?? ''}/conn=${t.t.connected}; trace=${trace.join(' | ')}`)
      }
      for (const t of tabs) { t.unsub(); t.client.destroy(); t.undo.destroy(); t.doc.destroy() }
      channels.clear()
      server.doc.destroy(); server.durableDoc.destroy()
      vi.clearAllTimers()
    }
    console.log('R6-MODEL', JSON.stringify(stats))
    expect(stats.fitsButNotSaved, 'a state that fits returns to Saved once faults stop').toEqual([])
    expect(stats.byteBudgetFailures, 'one full retry per refusal or deletion change on each connection').toEqual([])
    expect(stats.deliveryFailures, 'all device edits eventually reach durable history after retryable faults stop').toEqual([])
    expect(stats.livenessFailures, 'durable device edits eventually become Saved after faults stop').toEqual([])
  }, 24 * 3600 * 1000)
})
