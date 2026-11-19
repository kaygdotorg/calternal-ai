import { afterEach, beforeEach, expect, test, vi } from 'vitest'
import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import { LiveSocket } from '../src/sync/live'

const JOINED = { id: 'a', lineage: 'l1', text: 'md' }

class Socket {
  static OPEN = 1
  static instances: Socket[] = []
  readyState = 1
  binaryType = ''
  onopen: (() => void) | null = null
  onmessage: ((ev: { data: string | ArrayBuffer }) => void) | null = null
  onclose: (() => void) | null = null
  sent: Array<string | Uint8Array> = []
  constructor(_url: string) { Socket.instances.push(this) }
  send(data: string | Uint8Array) { this.sent.push(data) }
  close() { this.readyState = 3; this.onclose?.() }
  json(data: unknown) { this.onmessage?.({ data: JSON.stringify(data) }) }
}

function latestMembership(ws: Socket): number {
  for (const sent of [...ws.sent].reverse()) {
    if (typeof sent !== 'string') continue
    const message = JSON.parse(sent) as { type: string; membership?: number }
    if (message.type === 'join') return message.membership!
  }
  throw new Error('no room join')
}
function packet(membership: number, bytes = [101, 1, 1]): ArrayBuffer {
  const e = encoding.createEncoder()
  encoding.writeVarString(e, `@${membership}/notes/a`)
  encoding.writeUint8Array(e, new Uint8Array(bytes))
  return encoding.toUint8Array(e).buffer as ArrayBuffer
}

beforeEach(() => {
  Socket.instances = []
  vi.stubGlobal('WebSocket', Socket)
})
afterEach(() => vi.unstubAllGlobals())

test('all large-message chunks carry their number and exact whole byte length', () => {
  const live = new LiveSocket(() => 'ws://test')
  live.connect()
  const ws = Socket.instances[0]!
  const bytes = new Uint8Array(1024 * 1024 + 7)
  bytes[0] = 1
  bytes[bytes.length - 1] = 9
  live.sendFrame('@1/notes/a', bytes)
  const chunks = ws.sent.map((sent) => {
    const d = decoding.createDecoder(sent as Uint8Array)
    return { room: decoding.readVarString(d), bytes: decoding.readTailAsUint8Array(d) }
  })
  expect(chunks.map((c) => c.room)).toEqual([
    '+0/1048583/@1/notes/a', '+1/1048583/@1/notes/a', '+2/1048583/@1/notes/a',
  ])
  expect(chunks.map((c) => c.bytes.length)).toEqual([512 * 1024, 512 * 1024, 7])
  expect(chunks[0]!.bytes[0]).toBe(1)
  expect(chunks[2]!.bytes[6]).toBe(9)
  live.disconnect()
})

test('small and empty messages keep their ordinary room envelope', () => {
  const live = new LiveSocket(() => 'ws://test')
  live.connect()
  const ws = Socket.instances[0]!
  live.sendFrame('notes/a', new Uint8Array([1, 2]))
  live.sendFrame('notes/a', new Uint8Array())
  for (const sent of ws.sent) {
    const d = decoding.createDecoder(sent as Uint8Array)
    expect(decoding.readVarString(d)).toBe('notes/a')
  }
  live.disconnect()
})

test('a server size refusal reaches the sync error callback before closing the room', () => {
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const events: string[] = []
  room.onError?.((code) => events.push(code))
  room.onClose(() => events.push('closed'))
  room.connect()
  const ws = Socket.instances[0]!
  ws.json({ type: 'joined', room: 'notes/a', membership: latestMembership(ws), info: JOINED })
  expect(room.connected).toBe(true)
  ws.json({ type: 'too_large', room: 'notes/a', membership: latestMembership(ws), error: { code: 'too_large' } })
  expect(events).toEqual(['too_large', 'closed'])
  expect(room.connected).toBe(false)
  const count = ws.sent.length
  room.send(new Uint8Array([1]))
  expect(ws.sent).toHaveLength(count)
  live.disconnect()
})


test('callbacks from a replaced websocket cannot affect the new room connection', () => {
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const errors: string[] = []
  const messages: number[] = []
  let closes = 0, opens = 0
  live.onOpen(() => opens++)
  room.onError?.((code) => errors.push(code))
  room.onClose(() => closes++)
  room.onMessage((bytes) => messages.push(bytes[0]!))
  room.connect()
  const old = Socket.instances[0]!
  old.onopen?.()
  old.json({ type: 'joined', room: 'notes/a', membership: latestMembership(old), info: JOINED })
  const oldOpen = old.onopen!, oldMessage = old.onmessage!, oldClose = old.onclose!
  live.disconnect()
  live.connect()
  const current = Socket.instances[1]!
  current.onopen?.()
  current.json({ type: 'joined', room: 'notes/a', membership: latestMembership(current), info: JOINED })
  const closeCount = closes
  const receipt = packet(latestMembership(current))
  oldMessage({ data: receipt })
  oldMessage({ data: JSON.stringify({ type: 'too_large', room: 'notes/a', membership: latestMembership(current) }) })
  oldClose()
  oldOpen()
  expect(errors).toEqual([])
  expect(messages).toEqual([])
  expect(closes).toBe(closeCount)
  expect(opens).toBe(2)
  expect(room.connected).toBe(true)
  current.onmessage?.({ data: receipt })
  expect(messages).toEqual([101])
  live.disconnect()
})


test('same-socket room replacement ignores already-sent packets and controls from the old membership', () => {
  const live = new LiveSocket(() => 'ws://test')
  const old = live.room('notes/a')
  old.connect()
  const ws = Socket.instances[0]!
  const previous = latestMembership(ws)
  ws.json({ type: 'joined', room: 'notes/a', membership: previous, info: JOINED })
  old.disconnect()
  const current = live.room('notes/a')
  const messages: number[] = [], errors: string[] = []
  current.onMessage((bytes) => messages.push(bytes[0]!))
  current.onError?.((code) => errors.push(code))
  current.connect()
  const membership = latestMembership(ws)
  expect(membership).toBeGreaterThan(previous)
  ws.json({ type: 'joined', room: 'notes/a', membership: previous, info: { ...JOINED, lineage: 'old' } })
  ws.json({ type: 'joined', room: 'notes/a', info: JOINED })
  expect(current.connected).toBe(false)
  ws.onmessage?.({ data: packet(previous) })
  ws.json({ type: 'joined', room: 'notes/a', membership, info: { ...JOINED, lineage: 'current' } })
  ws.onmessage?.({ data: packet(previous) })
  ws.json({ type: 'left', room: 'notes/a', membership: previous })
  ws.json({ type: 'too_large', room: 'notes/a', membership: previous })
  ws.json({ type: 'join_failed', room: 'notes/a', membership: previous, error: { code: 'forbidden' } })
  expect(current.connected).toBe(true)
  expect(errors).toEqual([])
  expect(messages).toEqual([])
  ws.onmessage?.({ data: packet(membership) })
  expect(messages).toEqual([101])
  current.send(new Uint8Array([0, 1]))
  const outgoing = ws.sent.at(-1) as Uint8Array
  expect(decoding.readVarString(decoding.createDecoder(outgoing))).toBe(`@${membership}/notes/a`)
  current.disconnect()
  expect(JSON.parse(ws.sent.at(-1) as string)).toEqual({ type: 'leave', room: 'notes/a', membership })
  live.disconnect()
})

test.each([
  ['the code itself', { code: 'lineage_changed' }],
  ['conflict with details.reason', { code: 'conflict', details: { reason: 'lineage_changed' } }],
])('a lineage_changed refusal (%s) closes the room as a lineage mismatch and is not retried', (_name, error) => {
  vi.useFakeTimers()
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const closes: Array<[number, string]> = []
  room.onClose((code, reason) => closes.push([code, reason]))
  room.connect()
  const ws = Socket.instances[0]!
  const joins = () => ws.sent.filter((s) => typeof s === 'string' && JSON.parse(s).type === 'join').length
  ws.json({ type: 'join_failed', room: 'notes/a', membership: latestMembership(ws), error })
  expect(closes).toEqual([[4409, 'lineage_changed']])
  vi.advanceTimersByTime(5000)
  expect(joins()).toBe(1)
  live.disconnect()
  vi.useRealTimers()
})

test('any other conflict is not mistaken for a lineage change', () => {
  vi.useFakeTimers()
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const closes: Array<[number, string]> = []
  room.onClose((code, reason) => closes.push([code, reason]))
  room.connect()
  const ws = Socket.instances[0]!
  ws.json({ type: 'join_failed', room: 'notes/a', membership: latestMembership(ws), error: { code: 'conflict', details: { reason: 'something_else' } } })
  expect(closes).toEqual([])
  live.disconnect()
  vi.useRealTimers()
})

function piece(membership: number, seq: number, total: number, bytes: Uint8Array): ArrayBuffer {
  const e = encoding.createEncoder()
  encoding.writeVarString(e, `+${seq}/${total}/@${membership}/notes/a`)
  encoding.writeUint8Array(e, bytes)
  return encoding.toUint8Array(e).buffer as ArrayBuffer
}
function joinedRoom() {
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const got: Uint8Array[] = []
  room.onMessage((b) => got.push(b))
  room.connect()
  const ws = Socket.instances[0]!
  const membership = latestMembership(ws)
  ws.json({ type: 'joined', room: 'notes/a', membership, info: JOINED })
  return { live, room, ws, membership, got }
}

test('a payload over 512 KiB arrives in pieces and is decoded once, whole', () => {
  const { live, ws, membership, got } = joinedRoom()
  const whole = new Uint8Array(1024 * 1024 + 7).map((_, i) => i % 251)
  const total = whole.length, P = 512 * 1024
  ws.onmessage?.({ data: piece(membership, 0, total, whole.subarray(0, P)) })
  ws.onmessage?.({ data: piece(membership, 1, total, whole.subarray(P, 2 * P)) })
  expect(got).toHaveLength(0)
  ws.onmessage?.({ data: piece(membership, 2, total, whole.subarray(2 * P)) })
  expect(got).toHaveLength(1)
  expect(got[0]!.length).toBe(total)
  expect(Array.from(got[0]!.subarray(0, 4))).toEqual([0, 1, 2, 3])
  expect(got[0]![total - 1]).toBe(whole[total - 1])
  live.disconnect()
})

test('pieces out of order, for another total, or too many bytes are dropped, never delivered', () => {
  const { live, ws, membership, got } = joinedRoom()
  const P = 512 * 1024, a = new Uint8Array(P)
  ws.onmessage?.({ data: piece(membership, 1, 2 * P + 10, a) }) // starts at 1
  ws.onmessage?.({ data: piece(membership, 0, 2 * P + 10, a) })
  ws.onmessage?.({ data: piece(membership, 2, 2 * P + 10, a) }) // skips 1
  ws.onmessage?.({ data: piece(membership, 0, 2 * P + 10, a) })
  ws.onmessage?.({ data: piece(membership, 1, 3 * P, a) }) // another total
  ws.onmessage?.({ data: piece(membership, 0, 64 * 1024 * 1024 + 1, a) }) // over the cap
  expect(got).toHaveLength(0)
  live.disconnect()
})

test('a reassembly that waits 15 seconds for its next piece is dropped', () => {
  vi.useFakeTimers()
  const { live, ws, membership, got } = joinedRoom()
  const P = 512 * 1024, a = new Uint8Array(P)
  ws.onmessage?.({ data: piece(membership, 0, P + 4, a) })
  vi.advanceTimersByTime(15_001)
  ws.onmessage?.({ data: piece(membership, 1, P + 4, new Uint8Array(4)) })
  expect(got).toHaveLength(0)
  live.disconnect()
  vi.useRealTimers()
})

test.each([
  ['no lineage', { id: 'a', text: 'md' }],
  ['another item', { id: 'b', lineage: 'l1', text: 'md' }],
  ['text that is not md', { id: 'a', lineage: 'l1', text: 'bin' }],
  ['no info', undefined],
])('a joined answer with %s is refused and the room never opens', (_n, info) => {
  const live = new LiveSocket(() => 'ws://test')
  const room = live.room('notes/a')
  const closes: Array<[number, string]> = [], errors: string[] = []
  room.onClose((c, r) => closes.push([c, r]))
  room.onError?.((e) => errors.push(e))
  room.connect()
  const ws = Socket.instances[0]!
  const membership = latestMembership(ws)
  ws.json({ type: 'joined', room: 'notes/a', membership, info })
  expect(room.connected).toBe(false)
  expect(closes).toEqual([[4400, 'invalid_join']])
  expect(errors).toEqual(['invalid_join'])
  expect(JSON.parse(ws.sent.at(-1) as string)).toEqual({ type: 'leave', room: 'notes/a', membership })
  live.disconnect()
})

test('a join refused as not_found after having been admitted is an access revoked while away (4403),', () => {
  vi.useFakeTimers()
  const { live, room, ws } = joinedRoom()
  const closes: Array<[number, string]> = []
  room.onClose((c, r) => closes.push([c, r]))
  ws.json({ type: 'left', room: 'notes/a', membership: latestMembership(ws), reason: 'fell_behind' })
  vi.advanceTimersByTime(200)
  const again = latestMembership(ws)
  ws.json({ type: 'join_failed', room: 'notes/a', membership: again, error: { code: 'not_found', message: 'gone' } })
  expect(closes.at(-1)).toEqual([4403, 'access_revoked'])
  vi.advanceTimersByTime(10_000)
  expect(ws.sent.filter((s) => typeof s === 'string' && JSON.parse(s).type === 'join')).toHaveLength(2)
  live.disconnect()
  vi.useRealTimers()
})

test('left with access_revoked ends the membership for good: closed with 4403, never joined again, socket kept', () => {
  vi.useFakeTimers()
  const { live, room, ws, membership } = joinedRoom()
  const closes: Array<[number, string]> = []
  room.onClose((c, r) => closes.push([c, r]))
  ws.json({ type: 'left', room: 'notes/a', membership, reason: 'access_revoked' })
  expect(closes).toEqual([[4403, 'access_revoked']])
  vi.advanceTimersByTime(10_000)
  expect(ws.sent.filter((s) => typeof s === 'string' && JSON.parse(s).type === 'join')).toHaveLength(1)
  expect(ws.readyState).toBe(1)
  live.disconnect()
  vi.useRealTimers()
})
