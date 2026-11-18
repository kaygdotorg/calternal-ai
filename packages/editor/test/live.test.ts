import { afterEach, beforeEach, expect, test, vi } from 'vitest'
import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import { LiveSocket } from '../src/sync/live'

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
  ws.json({ type: 'joined', room: 'notes/a', membership: latestMembership(ws), info: {} })
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
  old.json({ type: 'joined', room: 'notes/a', membership: latestMembership(old), info: {} })
  const oldOpen = old.onopen!, oldMessage = old.onmessage!, oldClose = old.onclose!
  live.disconnect()
  live.connect()
  const current = Socket.instances[1]!
  current.onopen?.()
  current.json({ type: 'joined', room: 'notes/a', membership: latestMembership(current), info: {} })
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
  ws.json({ type: 'joined', room: 'notes/a', membership: previous, info: {} })
  old.disconnect()
  const current = live.room('notes/a')
  const messages: number[] = [], errors: string[] = []
  current.onMessage((bytes) => messages.push(bytes[0]!))
  current.onError?.((code) => errors.push(code))
  current.connect()
  const membership = latestMembership(ws)
  expect(membership).toBeGreaterThan(previous)
  ws.json({ type: 'joined', room: 'notes/a', membership: previous, info: { lineage: 'old' } })
  ws.json({ type: 'joined', room: 'notes/a', info: {} })
  expect(current.connected).toBe(false)
  ws.onmessage?.({ data: packet(previous) })
  ws.json({ type: 'joined', room: 'notes/a', membership, info: { lineage: 'current' } })
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
