/**
 * The one live socket (ADR 0012), and note rooms on it.
 *
 * Each client keeps a single websocket to `/api/v1/live`. It carries JSON
 * text messages (journal entries, action results, room joins) and binary
 * room frames: `@<membership>/<room>` as a lib0 var-string, then one y-protocols
 * message. See `crates/server/src/rooms.rs` for the server's side.
 *
 * A room is joined with `{"type":"join","room":"notes/<id>","membership":1,"lineage":…}`.
 * The server answers `joined` (with the room's lineage) before any frame,
 * or `join_failed`; a `lineage_changed` refusal (the code itself, or `conflict` with `details.reason` `lineage_changed`) means this device's copy is from an
 * older lineage and must not be merged. `left` means the server dropped us
 * from the room; we join again and resync. Every join gets a fresh page-wide
 * membership nonce; JSON and binary packets from older joins are ignored.
 *
 * The server takes socket messages up to 1 MiB. A larger room message (a
 * big paste) goes in pieces: every piece is framed for the
 * room `+<sequence>/<total bytes>/@<membership>/<room>`, including the last piece.
 * Sequences start at zero; the server checks sequence and total length up to
 * `MAX_ROOM_MESSAGE`.
 */

import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import { CLOSE_LINEAGE, CLOSE_REVOKED, MAX_ROOM_MESSAGE } from './protocol'

/** Room message bytes per socket message, safely under the server's 1 MiB. */
const PIECE = 512 * 1024
/** The longest a reassembly waits for its next piece. */
const PIECE_GAP_MS = 15_000
import type { Transport } from './transport'

/** A JSON message on the live socket. */
export interface LiveMessage {
  type: string
  [key: string]: unknown
}

/** Close codes a room transport reports, besides 1000 (we left). */
export const CLOSE_DROPPED = 1006
export const CLOSE_REFUSED = 4400

// Shared by every LiveSocket/room instance in this page. A room replacement
// never reuses the identity of packets already on the wire.
let membershipSequence = 0

const BACKOFF_MS = [100, 250, 500, 1000, 2000, 5000]

/** Join refusals that another try won't fix. */
const FINAL = new Set(['not_found', 'unauthorized', 'forbidden', 'invalid_input'])

export class LiveSocket {
  private ws: WebSocket | null = null
  private wanted = false
  private attempt = 0
  private timer: ReturnType<typeof setTimeout> | null = null
  private readonly jsonFns = new Set<(m: LiveMessage) => void>()
  private readonly openFns = new Set<() => void>()
  private readonly closeFns = new Set<() => void>()
  private readonly rooms = new Map<string, RoomTransport>()

  /** `url` is asked on every (re)connect, so it can carry a fresh `since`. `protocols` is the subprotocol offered on the upgrade (`calternal-live-rooms.v1`). */
  constructor(private readonly url: () => string, private readonly protocols?: string | string[]) {}

  get open(): boolean {
    return this.ws?.readyState === WebSocket.OPEN
  }

  connect(): void {
    if (this.wanted) return
    this.wanted = true
    this.attempt = 0
    this.dial()
  }

  disconnect(): void {
    this.wanted = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = null
    this.ws?.close(1000)
  }

  send(msg: LiveMessage): void {
    if (this.open) this.ws!.send(JSON.stringify(msg))
  }

  sendFrame(room: string, bytes: Uint8Array): void {
    if (!this.open) return
    if (bytes.length <= PIECE) {
      this.ws!.send(frame(room, bytes))
      return
    }
    for (let at = 0, seq = 0; at < bytes.length; at += PIECE, seq++) {
      this.ws!.send(frame(`+${seq}/${bytes.length}/${room}`, bytes.subarray(at, at + PIECE)))
    }
  }

  onJson(fn: (m: LiveMessage) => void): () => void {
    this.jsonFns.add(fn)
    return () => this.jsonFns.delete(fn)
  }

  onOpen(fn: () => void): () => void {
    this.openFns.add(fn)
    return () => this.openFns.delete(fn)
  }

  onClose(fn: () => void): () => void {
    this.closeFns.add(fn)
    return () => this.closeFns.delete(fn)
  }

  /** A transport for one room on this socket. */
  room(name: string, options: RoomOptions = {}): Transport {
    const t = new RoomTransport(this, name, options)
    this.rooms.set(name, t)
    return t
  }

  /** @internal */
  forget(name: string, t: RoomTransport): void {
    if (this.rooms.get(name) === t) this.rooms.delete(name)
  }

  /** Hands one whole room message to its room: `wire` is `@<membership>/<room>`. */
  private deliver(wire: string, bytes: Uint8Array) {
    const slash = wire.indexOf('/')
    if (!wire.startsWith('@') || slash < 2) return
    const membership = Number(wire.slice(1, slash))
    if (!Number.isSafeInteger(membership) || membership <= 0) return
    this.rooms.get(wire.slice(slash + 1))?.frame(bytes, membership)
  }

  /** The one message being reassembled from `+<sequence>/<total bytes>/@<membership>/<room>` pieces (the server splits payloads over `PIECE`), if any. */
  private assembly: { wire: string; total: number; next: number; parts: Uint8Array[]; have: number; timer: ReturnType<typeof setTimeout> } | null = null

  /** One continuation piece. A message is delivered only when all `total` bytes have arrived, in order, for the same membership and room; anything out of order, over the cap, over a piece, or silent for 15 s is dropped whole. */
  private piece(header: string, bytes: Uint8Array) {
    const m = /^\+(\d+)\/(\d+)\/(@\d+\/.+)$/.exec(header)
    if (!m || bytes.length > PIECE) return this.dropAssembly()
    const seq = Number(m[1]), total = Number(m[2]), wire = m[3]!
    let a = this.assembly
    if (seq === 0) {
      if (a || total > MAX_ROOM_MESSAGE || total <= PIECE) return this.dropAssembly()
      a = this.assembly = { wire, total, next: 0, parts: [], have: 0, timer: 0 as never }
    } else if (!a || seq !== a.next || wire !== a.wire || total !== a.total) return this.dropAssembly()
    clearTimeout(a.timer)
    a.parts.push(bytes)
    a.have += bytes.length
    a.next++
    if (a.have > a.total) return this.dropAssembly()
    if (a.have < a.total) { a.timer = setTimeout(() => this.dropAssembly(), PIECE_GAP_MS); return }
    const whole = new Uint8Array(a.total)
    let at = 0
    for (const p of a.parts) { whole.set(p, at); at += p.length }
    this.assembly = null
    this.deliver(a.wire, whole)
  }

  private dropAssembly() {
    if (this.assembly) clearTimeout(this.assembly.timer)
    this.assembly = null
  }

  private dial() {
    this.timer = null
    if (!this.wanted) return
    const ws = new WebSocket(this.url(), this.protocols)
    ws.binaryType = 'arraybuffer'
    this.ws = ws
    ws.onopen = () => {
      if (this.ws !== ws) return
      this.attempt = 0
      this.openFns.forEach((f) => f())
      this.rooms.forEach((r) => r.socketOpened())
    }
    ws.onmessage = (ev) => {
      if (this.ws !== ws) return
      if (typeof ev.data === 'string') {
        let msg: LiveMessage
        try {
          msg = JSON.parse(ev.data) as LiveMessage
        } catch {
          return
        }
        if (typeof msg.room === 'string') this.rooms.get(msg.room)?.json(msg)
        this.jsonFns.forEach((f) => f(msg))
      } else if (ev.data instanceof ArrayBuffer) {
        const d = decoding.createDecoder(new Uint8Array(ev.data))
        const wire = decoding.readVarString(d)
        const rest = decoding.readTailAsUint8Array(d)
        if (wire.startsWith('+')) return this.piece(wire, rest)
        this.deliver(wire, rest)
      }
    }
    ws.onclose = () => {
      if (this.ws !== ws) return
      this.ws = null
      this.dropAssembly()
      this.rooms.forEach((r) => r.socketClosed())
      this.closeFns.forEach((f) => f())
      if (this.wanted && !this.timer) {
        const delay = BACKOFF_MS[Math.min(this.attempt++, BACKOFF_MS.length - 1)]
        this.timer = setTimeout(() => this.dial(), delay)
      }
    }
  }
}

/** A binary frame: the room name as a lib0 var-string, then the bytes. */
function frame(room: string, bytes: Uint8Array) {
  const e = encoding.createEncoder()
  encoding.writeVarString(e, room)
  encoding.writeUint8Array(e, bytes)
  return encoding.toUint8Array(e)
}

export interface RoomOptions {
  /** The lineage of the copy this device holds, asked at every join. */
  lineage?: () => string | null
  /** The room's lineage, told on every accepted join, before any frame. */
  onLineage?: (lineage: string) => void
  /** The person's timezone offset, like `+05:30`. */
  tz?: string
}

/** One room on the live socket, as a sync transport. */
class RoomTransport implements Transport {
  private wanted = false
  private joined = false
  /** Whether this socket has ever been admitted: a later join the room refuses as `not_found` or `forbidden` then means access was taken away while the connection was down. */
  private admitted = false
  private joining = false
  private membership = 0
  private retry: ReturnType<typeof setTimeout> | null = null
  private readonly opens: Array<() => void> = []
  private readonly closes: Array<(code: number, reason: string) => void> = []
  private readonly messages: Array<(data: Uint8Array) => void> = []
  private readonly errors: Array<(code: string) => void> = []

  constructor(
    private readonly socket: LiveSocket,
    private readonly name: string,
    private readonly options: RoomOptions,
  ) {}

  get connected(): boolean {
    return this.joined
  }

  connect(): void {
    this.wanted = true
    this.socket.connect()
    if (this.socket.open && !this.joined && !this.joining) this.join()
  }

  disconnect(): void {
    this.wanted = false
    if (this.retry) clearTimeout(this.retry)
    this.retry = null
    if (this.joined || this.joining) {
      this.socket.send({ type: 'leave', room: this.name, membership: this.membership })
      this.joining = false
      const wasJoined = this.joined
      this.joined = false
      if (wasJoined) this.closes.forEach((f) => f(1000, ''))
    }
  }

  send(data: Uint8Array): void {
    if (this.joined) this.socket.sendFrame(`@${this.membership}/${this.name}`, data)
  }

  onOpen = (f: () => void) => void this.opens.push(f)
  onClose = (f: (code: number, reason: string) => void) => void this.closes.push(f)
  onMessage = (f: (data: Uint8Array) => void) => void this.messages.push(f)
  onError = (f: (code: string) => void) => void this.errors.push(f)

  // --- from the socket ------------------------------------------------------

  socketOpened() {
    if (this.wanted && !this.joined && !this.joining) this.join()
  }

  socketClosed() {
    this.joining = false
    if (!this.joined) return
    this.joined = false
    this.closes.forEach((f) => f(CLOSE_DROPPED, ''))
  }

  frame(data: Uint8Array, membership: number) {
    if (this.joined && membership === this.membership) this.messages.forEach((f) => f(data))
  }

  json(msg: LiveMessage) {
    if (msg.membership !== this.membership) return
    switch (msg.type) {
      case 'joined': {
        if (!this.wanted) {
          this.socket.send({ type: 'leave', room: this.name, membership: this.membership })
          return
        }
        // The room's answer must be for this item, in the shape the contract promises, before a single frame is applied: a lineage, the item's id, and the text kind `md`.
        const info = msg.info as { id?: unknown; lineage?: unknown; text?: unknown } | undefined
        if (typeof info?.lineage !== 'string' || !info.lineage || info.id !== this.name.slice(this.name.lastIndexOf('/') + 1) || info.text !== 'md') {
          this.socket.send({ type: 'leave', room: this.name, membership: this.membership })
          this.errors.forEach((f) => f('invalid_join'))
          return this.refused(CLOSE_REFUSED, 'invalid_join')
        }
        const mine = this.options.lineage?.() ?? null
        const theirs = info.lineage
        if (mine && theirs && mine !== theirs) {
          this.socket.send({ type: 'leave', room: this.name, membership: this.membership })
          return this.refused(CLOSE_LINEAGE, theirs)
        }
        if (theirs) this.options.onLineage?.(theirs)
        this.joining = false
        this.joined = true
        this.admitted = true
        this.opens.forEach((f) => f())
        return
      }
      case 'too_large':
        this.joining = false
        this.errors.forEach((f) => f('too_large'))
        this.joined = false
        this.closes.forEach((f) => f(CLOSE_REFUSED, 'too_large'))
        return
      case 'room_error':
        this.errors.forEach((f) => f((msg.error as { code?: string } | undefined)?.code ?? 'room_error'))
        return
      case 'join_failed': {
        this.joining = false
        const error = msg.error as { code?: string; details?: { reason?: string } } | undefined
        const code = error?.code ?? ''
        // Two shapes of the same refusal: the code itself (`lineage_changed`), or `conflict` with `details.reason` `lineage_changed` (docs/live-rooms.md, "Browser binding").
        if (code === 'lineage_changed' || (code === 'conflict' && error?.details?.reason === 'lineage_changed')) return this.refused(CLOSE_LINEAGE, 'lineage_changed')
        this.errors.forEach((f) => f(code || 'join_failed'))
        // Admitted before, refused now as not found or forbidden: access was revoked while offline, the same end as a `left` with `access_revoked` (the host rescues what this person wrote).
        if (this.admitted && (code === 'not_found' || code === 'forbidden')) return this.refused(CLOSE_REVOKED, 'access_revoked')
        if (FINAL.has(code)) return this.refused(CLOSE_REFUSED, code)
        // Anything else (a server hiccup) is worth another try.
        if (this.wanted) this.retry = setTimeout(() => this.connect(), 1000)
        return
      }
      case 'left':
        this.joining = false
        // Access was taken away: the membership is over for good (the socket stays), and the host keeps what the person wrote (`CLOSE_REVOKED`).
        if (msg.reason === 'access_revoked') return this.refused(CLOSE_REVOKED, 'access_revoked')
        if (!this.joined) return
        this.joined = false
        this.closes.forEach((f) => f(CLOSE_DROPPED, ''))
        // Dropped for falling behind: join again and resync.
        if (this.wanted) this.retry = setTimeout(() => this.connect(), 100)
        return
    }
  }

  private join() {
    this.joining = true
    this.membership = ++membershipSequence
    const lineage = this.options.lineage?.() ?? null
    this.socket.send({ type: 'join', room: this.name, membership: this.membership, tz: this.options.tz, ...(lineage ? { lineage } : {}) })
  }

  private refused(code: number, reason: string) {
    this.wanted = false
    this.joining = false
    this.joined = false
    this.closes.forEach((f) => f(code, reason))
  }
}
