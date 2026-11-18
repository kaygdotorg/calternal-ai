/**
 * Refusals must not re-upload whole notes, and offline receipts must yield
 * through MessageChannel. Backlog wall/CPU qualification belongs to the
 * isolated perf/receipt-rank.ts harness; work bounds have an operation-count
 * regression in receipt-status-work.test.ts.
 */
import { afterEach, expect, it, vi } from 'vitest'
import * as Y from 'yjs'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { decodeMessage, decodeSaveCheck, encodeSaved, encodeStep1, encodeStep2, encodeUpdate } from '../src/sync/protocol'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

/** Real SyncClient on a hand-driven transport; save checks are sent by calling `check()`. */
function fixture(initialText = '', refused: 'error' | null = null) {
  const doc = new Y.Doc()
  if (initialText) doc.getText('md').insert(0, initialText)
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
  const client = new SyncClient({ doc, transport, gate: new ImeGate(), channel: null, refused })
  const check = () => (client as unknown as { sendCheck(): void }).sendCheck()
  const server = new Y.Doc()
  const receipts: number[] = []
  /** Answers every save check sent so far; returns how long each answer took. */
  const answer = () => {
    for (const p of packets.splice(0)) {
      const id = decodeSaveCheck(p)
      if (id == null) continue
      const start = performance.now(); message(encodeSaved(id)); receipts.push(performance.now() - start)
    }
  }
  client.connect(); message(encodeStep1(server)); message(encodeStep2(server, Y.encodeStateVector(doc)))
  const bytes = (kinds = ['update', 'step2']) => packets.filter((p) => kinds.includes(decodeMessage(p).kind)).reduce((n, p) => n + p.length, 0)
  return { doc, client, check, message, packets, answer, receipts, bytes, destroy() { client.destroy(); doc.destroy(); server.destroy() } }
}

/** Counts receipt tasks; timing is optional when only scheduling correctness is under test. */
function timeChannels(measure = true) {
  const Real = globalThis.MessageChannel
  const tasks: number[] = []
  let posts = 0
  // Receipts only use port1.onmessage/close and port2.postMessage.
  function Timed() {
    const real = new Real()
    return {
      port1: {
        set onmessage(fn: ((ev: MessageEvent) => void) | null) {
          real.port1.onmessage = fn && ((ev: MessageEvent) => { const start = measure ? performance.now() : 0; fn(ev); tasks.push(measure ? performance.now() - start : 0) })
        },
        close: () => real.port1.close(),
      },
      port2: { postMessage: (m: unknown) => { posts++; real.port2.postMessage(m) }, close: () => real.port2.close() },
    }
  }
  vi.stubGlobal('MessageChannel', Timed)
  return { tasks, get posts() { return posts } }
}

/**
 * Timings on a shared, loaded host can catch one garbage-collection pause.
 * A round counts if none of its callbacks reached 16 ms; up to three rounds
 * are tried. A real regression (a re-encode) takes hundreds of ms every time.
 */
async function bestRound(run: () => number[] | Promise<number[]>): Promise<number[]> {
  const rounds: number[] = []
  for (let i = 0; i < 3; i++) {
    const max = Math.max(...await run())
    rounds.push(max)
    if (max < 16) break
  }
  return rounds
}

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

it('20 keys on a 3.2 KB note with a refusal pending upload one full copy plus the edits', () => {
  const f = fixture('x'.repeat(3200))
  try {
    f.check(); f.answer()
    expect(f.client.status.get().state).toBe('saved')
    f.client.refuse('error', true)
    f.packets.length = 0
    const copy = encodeUpdate(Y.encodeStateAsUpdate(f.doc)).length
    let edits = 0
    f.doc.on('update', (u: Uint8Array) => { edits += encodeUpdate(u).length })
    // No receipt arrives while typing: the refusal stays pending throughout.
    for (let i = 0; i < 20; i++) { f.doc.getText('md').insert(0, 'k'); f.check() }
    const bytes = f.bytes()
    console.log('R7-BYTES', JSON.stringify({ copy, edits, bytes }))
    expect(bytes).toBeLessThanOrEqual(copy + edits + 20)
    expect(f.client.status.get().state).not.toBe('saved')
    // Its receipt still clears the refusal.
    f.answer()
    expect(f.client.status.get().error).toBeUndefined()
  } finally { f.destroy() }
})

it.each(['ascii', 'unicode'])('typing and collaborator packets next to a refused 65 MiB paste stay under 16 ms (%s)', async (kind) => {
  const f = fixture()
  const peer = new Y.Doc()
  try {
    f.check(); f.answer()
    f.doc.getText('md').insert(0, kind === 'ascii' ? 'x'.repeat(65 * 1024 * 1024) : '€'.repeat(Math.ceil(65 * 1024 * 1024 / 3)))
    expect(f.client.status.get().tooLarge).toBe(true)
    f.packets.length = 0
    let local: number[] = [], remote: number[] = []
    const rounds = await bestRound(() => {
      local = []; remote = []; f.packets.length = 0
      for (let i = 0; i < 20; i++) {
        let start = performance.now()
        f.doc.getText('md').insert(0, 'k'); f.check()
        local.push(performance.now() - start)
        let update!: Uint8Array
        peer.once('update', (u: Uint8Array) => { update = u })
        peer.getText('md').insert(0, 'p')
        start = performance.now(); f.message(encodeUpdate(update)); f.check()
        remote.push(performance.now() - start)
      }
      return [...local, ...remote]
    })
    const bytes = f.bytes()
    console.log('R7-OVERSIZE', JSON.stringify({ kind, localMax: Math.max(...local), remoteMax: Math.max(...remote), rounds, bytes }))
    expect(rounds.at(-1)).toBeLessThan(16)
    expect(bytes).toBeLessThan(2000)
    expect(f.client.status.get().tooLarge).toBe(true)
  } finally { f.destroy(); peer.destroy() }
}, 180000)

it('a backspace next to a refused CJK paste (short in UTF-16, 72 MB in UTF-8) does not re-encode it', async () => {
  const f = fixture()
  try {
    f.check(); f.answer()
    const text = f.doc.getText('md')
    text.insert(0, '漢'.repeat(23 * 1024 * 1024))
    expect(f.client.status.get().tooLarge).toBe(true)
    f.check(); f.packets.length = 0
    // The lower bound must start at the paste's full UTF-8 size: no encode at the first deletion.
    const encode = vi.spyOn(f.client as unknown as { currentStatePacket(): Uint8Array }, 'currentStatePacket')
    const rounds = await bestRound(() => {
      const times: number[] = []
      for (let i = 0; i < 5; i++) {
        let start = performance.now(); text.insert(0, 'k'); f.check(); times.push(performance.now() - start)
        start = performance.now(); text.delete(0, 1); f.check(); times.push(performance.now() - start)
      }
      return times
    })
    console.log('R8-CJK', JSON.stringify({ rounds, encodes: encode.mock.calls.length, bytes: f.bytes() }))
    expect(encode).not.toHaveBeenCalled()
    expect(rounds.at(-1)).toBeLessThan(16)
    expect(f.client.status.get().tooLarge).toBe(true)
  } finally { f.destroy() }
}, 180000)

it('a reconnect next to a refused 65 MiB paste does not re-encode it', async () => {
  const f = fixture()
  try {
    f.check(); f.answer()
    f.doc.getText('md').insert(0, 'x'.repeat(65 * 1024 * 1024))
    f.packets.length = 0
    const rounds = await bestRound(() => {
      const start = performance.now()
      f.client.disconnect(); f.client.connect()
      f.message(encodeStep1(new Y.Doc()))
      return [performance.now() - start]
    })
    console.log('R7-RECONNECT', JSON.stringify({ rounds, bytes: f.bytes() }))
    expect(rounds.at(-1)).toBeLessThan(16)
    expect(f.bytes()).toBe(0)
    expect(f.client.status.get().tooLarge).toBe(true)
  } finally { f.destroy() }
}, 60000)

it('deleting the refused paste sends the current state once, and its receipt clears the refusal', () => {
  const f = fixture()
  try {
    f.check(); f.answer()
    const text = f.doc.getText('md')
    text.insert(0, 'keep')
    text.insert(4, 'y'.repeat(2 * 1024 * 1024))
    f.check(); f.answer()
    expect(f.client.status.get().state).toBe('saved')
    text.insert(4, 'x'.repeat(65 * 1024 * 1024))
    expect(f.client.status.get().tooLarge).toBe(true)
    f.packets.length = 0
    text.delete(4, 65 * 1024 * 1024)
    for (let i = 0; i < 5; i++) { text.insert(0, 'k'); f.check() }
    const full = f.packets.filter((p) => decodeMessage(p).kind === 'step2' && p.length > 1024 * 1024)
    expect(full).toHaveLength(1)
    f.answer()
    expect(f.client.status.get()).toMatchObject({ state: 'saved', tooLarge: false })
  } finally { f.destroy() }
}, 60000)

it('a large step-2 receipt proves coverage without re-applying the note, under 16 ms', async () => {
  const channels = timeChannels()
  const f = fixture('x'.repeat(63 * 1024 * 1024))
  try {
    f.check(); f.answer()
    for (let i = 0; i < 200 && !f.client.isConfirmed; i++) await new Promise((resolve) => setTimeout(resolve, 5))
    const max = Math.max(...f.receipts, ...channels.tasks)
    console.log('R7-LARGE-PROOF', JSON.stringify({ max, receipts: f.receipts, tasks: channels.tasks.length }))
    expect(f.client.isConfirmed).toBe(true)
    expect(max).toBeLessThan(16)
  } finally { f.destroy() }
}, 60000)

it('a 10,000-edit offline backlog reaches Saved through bounded MessageChannel steps', async () => {
  const channels = timeChannels(false)
  const zeroTimers: unknown[] = []
  const realTimeout = globalThis.setTimeout
  const spy = vi.spyOn(globalThis, 'setTimeout').mockImplementation(((fn: () => void, ms?: number, ...rest: unknown[]) => {
    if (!ms) zeroTimers.push(fn)
    return realTimeout(fn, ms, ...rest)
  }) as typeof setTimeout)
  const room = new Room('seed\n')
  room.checkDelayMs = 20
  const doc = new Y.Doc()
  const t = memoryTransport(() => room)
  const client = new SyncClient({ doc, transport: t, gate: new ImeGate(), checkDelayMs: 150 })
  const until = async (ok: () => boolean, what: string) => {
    for (let i = 0; i < 4000 && !ok(); i++) await new Promise((resolve) => realTimeout(resolve, 5))
    if (!ok()) throw new Error(`timeout: ${what}`)
  }
  try {
    client.connect()
    await until(() => client.isConfirmed, 'initial Saved')
    const text = doc.getText('md')
    t.drop()
    await until(() => !t.connected, 'offline')
    for (let i = 0; i < 10_000; i++) text.insert((i * 7919) % (text.length + 1), 'a')
    channels.tasks.length = 0; zeroTimers.length = 0
    const posts = channels.posts
    t.connect()
    await until(() => client.isConfirmed, 'backlog Saved')
    expect(client.status.get().state).toBe('saved')
    expect(room.doc.getText('md').toString()).toBe(text.toString())
    expect(channels.posts - posts).toBeGreaterThan(1)
    expect(zeroTimers).toHaveLength(0)
  } finally { spy.mockRestore(); client.destroy(); doc.destroy(); room.doc.destroy() }
}, 120000)
