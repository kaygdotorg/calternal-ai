/**
 * An in-memory stand-in for a note room, for unit tests only. It mirrors
 * what the real server (crates/notes room) does, so the sync client's unit
 * tests exercise the same contract the browser tests run against the real
 * binary:
 *
 * - on join: sync step 1 and the full state (step 2);
 * - updates go to every other client (never echoed to the sender);
 * - save checks (`100` → `101`) answered in order;
 * - a change with a gap is not applied: the room asks for a resend (step 1)
 *   and holds save checks until a sync step 2 fills it (`resync.ts`);
 * - presence (awareness, tag 1) is relayed to everyone else, a joiner is told who is here, and a client
 *   that leaves is removed from everyone's view;
 * - a client holding another lineage is refused (close 4409);
 * - only the server seeds a document from text;
 * - the lineage travels with the join answer only, never inside the
 *   document (like the real server: a fake that wrote it into the document
 *   once hid that the app never learned it).
 */

import * as decoding from 'lib0/decoding'
import * as encoding from 'lib0/encoding'
import * as awarenessProtocol from 'y-protocols/awareness'
import * as syncProtocol from 'y-protocols/sync'
import * as Y from 'yjs'
import { CLOSE_LINEAGE, MSG_AWARENESS, MSG_SYNC, decodeSaveCheck, encodeAwareness, encodeSaved, encodeUpdate } from '../src/sync/protocol'
import { followsOn, hasMissing } from './resync'

export interface Conn {
  send(data: Uint8Array): void
  close(code: number, reason: string): void
}

export class Room {
  doc = new Y.Doc()
  lineage = ''
  readonly conns = new Set<Conn>()
  /** The room's view of presence, and which client ids each connection owns. */
  private presence = new awarenessProtocol.Awareness(new Y.Doc())
  private owns = new Map<Conn, Set<number>>()
  /** Extra delay before answering save checks, to test "Saving…". */
  checkDelayMs = 0
  /** While true, save checks wait (as during a history outage) until `releaseChecks`. */
  holdChecks = false
  /** Save checks held, oldest first. */
  held: Array<() => void> = []
  /** Save checks received, in total. */
  checksReceived = 0
  /** Per connection: a resend was asked for, and the newest check held until it arrives. */
  private resyncs = new Map<Conn, { held: number | null }>()
  /** Pending check answers per connection; a resend request drops them (acks.rs). */
  private answers = new Map<Conn, Set<{ cancelled: boolean }>>()

  /** Answers the held checks, in order. */
  releaseChecks(): void {
    this.holdChecks = false
    const held = this.held
    this.held = []
    held.forEach((reply) => reply())
  }

  /** Answers only the newest held check: the server's answers are cumulative. */
  releaseNewest(): void {
    this.holdChecks = false
    const newest = this.held.at(-1)
    this.held = []
    newest?.()
  }

  constructor(text = '') {
    this.seed(text)
    this.presence.setLocalState(null)
  }

  get text(): string {
    return this.doc.getText('md').toString()
  }

  /** Starts a new lineage from `text`, as the server does from a file. */
  seed(text: string): void {
    this.doc.destroy()
    this.doc = new Y.Doc()
    this.lineage = Math.random().toString(36).slice(2, 10)
    if (text) this.doc.getText('md').insert(0, text)
    this.doc.on('update', (update: Uint8Array, origin: unknown) => {
      const msg = encodeUpdate(update)
      this.conns.forEach((c) => c !== origin && c.send(msg))
    })
    // Clients holding the old lineage are refused when they come back.
    for (const c of [...this.conns]) c.close(CLOSE_LINEAGE, this.lineage)
    this.conns.clear()
  }

  /** A client joins. Returns false if it was turned away. */
  join(conn: Conn, clientLineage: string | null): boolean {
    if (clientLineage && clientLineage !== this.lineage) {
      conn.close(CLOSE_LINEAGE, this.lineage)
      return false
    }
    this.conns.add(conn)
    const here = [...this.presence.getStates().keys()]
    if (here.length) conn.send(encodeAwareness(awarenessProtocol.encodeAwarenessUpdate(this.presence, here)))
    const step1 = encoding.createEncoder()
    encoding.writeVarUint(step1, MSG_SYNC)
    syncProtocol.writeSyncStep1(step1, this.doc)
    conn.send(encoding.toUint8Array(step1))
    const step2 = encoding.createEncoder()
    encoding.writeVarUint(step2, MSG_SYNC)
    syncProtocol.writeSyncStep2(step2, this.doc)
    conn.send(encoding.toUint8Array(step2))
    return true
  }

  leave(conn: Conn): void {
    const ids = [...(this.owns.get(conn) ?? [])]
    this.owns.delete(conn)
    if (ids.length) {
      awarenessProtocol.removeAwarenessStates(this.presence, ids, 'room')
      const gone = encodeAwareness(awarenessProtocol.encodeAwarenessUpdate(this.presence, ids))
      this.conns.forEach((c) => c !== conn && c.send(gone))
    }
    this.conns.delete(conn)
    this.resyncs.delete(conn)
    this.answers.delete(conn)
  }

  receive(conn: Conn, data: Uint8Array): void {
    const check = decodeSaveCheck(data)
    if (check != null) {
      this.checksReceived++
      const resync = this.resyncs.get(conn)
      if (resync) {
        // Held until the resend arrives; earlier answers are dropped.
        resync.held = check
        for (const a of this.answers.get(conn) ?? []) a.cancelled = true
        this.answers.delete(conn)
        return
      }
      this.answer(conn, check)
      return
    }
    const d = decoding.createDecoder(data)
    const tag = decoding.readVarUint(d)
    if (tag === MSG_AWARENESS) {
      const own = this.owns.get(conn) ?? this.owns.set(conn, new Set()).get(conn)!
      const onUpdate = ({ added, updated }: { added: number[]; updated: number[] }) => [...added, ...updated].forEach((id) => own.add(id))
      this.presence.on('update', onUpdate)
      awarenessProtocol.applyAwarenessUpdate(this.presence, decoding.readVarUint8Array(d), conn)
      this.presence.off('update', onUpdate)
      this.conns.forEach((c) => c !== conn && c.send(data))
      return
    }
    if (tag !== MSG_SYNC) return
    const type = decoding.readVarUint(d)
    if (type === syncProtocol.messageYjsSyncStep1) {
      const e = encoding.createEncoder()
      encoding.writeVarUint(e, MSG_SYNC)
      syncProtocol.writeSyncStep2(e, this.doc, decoding.readVarUint8Array(d))
      conn.send(encoding.toUint8Array(e))
      return
    }
    if (type !== syncProtocol.messageYjsSyncStep2 && type !== syncProtocol.messageYjsUpdate) return
    const update = decoding.readVarUint8Array(d)
    if (!followsOn(this.doc, update)) {
      this.resync(conn)
      return
    }
    Y.applyUpdate(this.doc, update, conn)
    if (hasMissing(this.doc)) this.resync(conn)
    else if (type === syncProtocol.messageYjsSyncStep2) this.resynced(conn)
  }

  /** Asks `conn` to resend everything the room lacks, holding its checks. */
  private resync(conn: Conn): void {
    if (this.resyncs.has(conn)) return
    this.resyncs.set(conn, { held: null })
    const e = encoding.createEncoder()
    encoding.writeVarUint(e, MSG_SYNC)
    syncProtocol.writeSyncStep1(e, this.doc)
    conn.send(encoding.toUint8Array(e))
  }

  private resynced(conn: Conn): void {
    const resync = this.resyncs.get(conn)
    this.resyncs.delete(conn)
    if (resync?.held != null) this.answer(conn, resync.held)
  }

  private answer(conn: Conn, check: number): void {
    const token = { cancelled: false }
    const answers = this.answers.get(conn) ?? this.answers.set(conn, new Set()).get(conn)!
    answers.add(token)
    const reply = () => {
      answers.delete(token)
      if (!token.cancelled && this.conns.has(conn)) conn.send(encodeSaved(check))
    }
    if (this.holdChecks) this.held.push(reply)
    else if (this.checkDelayMs) setTimeout(reply, this.checkDelayMs)
    else reply()
  }
}
