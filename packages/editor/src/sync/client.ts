/**
 * Keeps one note's document in step with the server and with other tabs.
 *
 * - Talks the y-websocket protocol (plus save checks, see `protocol.ts`).
 * - Changes from the server and from other tabs wait behind the IME gate,
 *   so an open composition on a phone keyboard is never disturbed.
 * - Tracks which changes the server has durably confirmed, so the save
 *   status never claims more than is true. The status is about this
 *   device's edits. The receipt authority also owns replay-base eligibility,
 *   which waits for all observed state; collaborators' receipts are batched.
 * - "Saved" answers are cumulative: an answer covers every earlier check.
 * - Tabs only share changes when they hold the same lineage, so a tab still
 *   on a replaced server copy can never feed it into the new one. A tab
 *   resends a sibling edit only if coverage remains missing; it counts as
 *   saved only by a server receipt covering it (`receipts.ts`).
 * - Updates larger than the server accepts (`MAX_ROOM_MESSAGE`) are never
 *   sent: they stay unsaved and the status says they are too large.
 * - After a refusal, the complete current state is sent again once, and
 *   again only after something could have made a too-large state fit (a
 *   deletion). Later edits travel as ordinary updates (`retryState`).
 * - Never seeds the document from file text: only the server does that.
 */

import * as awarenessProtocol from 'y-protocols/awareness'
import * as Y from 'yjs'
import type { ImeGate } from './gate'
import {
  CLOSE_LINEAGE,
  CLOSE_REVOKED,
  MAX_ROOM_MESSAGE,
  decodeMessage,
  encodeAwareness,
  encodeSaveCheck,
  encodeStep1,
  encodeStep2,
  encodeUpdate,
  permanentRefusal,
} from './protocol'
import { Receipts } from './receipts'
import { transactionProof, updateProof, type CoverageProof } from './coverage'
import { StatusBox } from './status'
import type { Transport } from './transport'

/** Origin used for changes that came from another tab. */
export const TAB_ORIGIN = Symbol('calternal-tab')

export interface SyncOptions {
  doc: Y.Doc
  transport: Transport
  gate: ImeGate
  /** BroadcastChannel name shared by this note's tabs, or null for none. */
  channel?: string | null
  /**
   * The lineage this document holds, or null before it has one. Tabs share
   * changes only once it is known, and only with tabs of the same lineage.
   */
  lineage?: () => string | null
  /** @deprecated Persistence updates need receipts like every other update. */
  quietOrigins?: unknown[]
  /**
   * When this device's oldest unconfirmed edit from an earlier visit was
   * made, or null when it has none. Only real local edits count.
   */
  pendingSince?: number | null
  /** Text held outside this document during a durable lineage replacement. */
  recovery?: Array<{ id: string; at: number }>
  /** How long to wait after a change before asking the server to confirm. */
  checkDelayMs?: number
  /**
   * How long collaborators' changes must be quiet before asking the server
   * to confirm them. Default 300 ms; during
   * nonstop changes it asks at most every 2 s (`BASE_CHECK_MAX_MS`).
   */
  baseCheckDelayMs?: number
  onPendingChange?: (since: number | null) => void
  /**
   * When the oldest change this device holds that the server hasn't
   * confirmed arrived, for changes that aren't this device's own edits
   * (a collaborator's). Signing out warns once this is two seconds old.
   */
  heldSince?: number | null
  onHeldChange?: (since: number | null) => void
  refused?: 'too_large' | 'error' | null
  onRefusalChange?: (code: 'too_large' | 'error' | null) => void
  /**
   * All state this device has observed has a durable server receipt.
   * This text can now become the safe replay base.
   */
  onConfirmed?: () => void
  onSynced?: () => void
  onLineageMismatch?: (serverLineage: string) => void
  /**
   * The room ended this membership because the person lost access to the note. The client stops for good (no reconnect); the host keeps what the person wrote that is not yet receipted (`sharing.rescue`) and says so.
   */
  onAccessRevoked?: () => void
  /**
   * Presence in this room (y-protocols awareness, tag 1): the local state is sent on every join and whenever it changes, other people's are applied, and everyone but this client is dropped when the
   * connection does. Cleared (`setLocalState(null)`) on `destroy`, so others see this person leave.
   */
  awareness?: awarenessProtocol.Awareness
}

/** Longest a collaborator's change waits before this device asks for its receipt. */
const BASE_CHECK_MAX_MS = 2000
/** How long another tab's edit may wait for that tab's receipt before this tab sends it. */
const SIBLING_WAIT_MS = 2000
/**
 * Most bytes one deleted Yjs unit (a UTF-16 code unit of text) can remove
 * from an encoded state: its own item header (info, two origin ids, length:
 * under 48 bytes) plus up to 3 bytes of UTF-8 text. Deleting never shrinks
 * an encoding by more, so a too-large state stays too large until enough
 * has been deleted (`oversize`).
 */
const FREED_PER_UNIT = 64
/** The state vector of an empty document: a step 2 for it is the complete state. */
const EMPTY_STATE_VECTOR = Y.encodeStateVector(new Y.Doc())

/** UTF-8 length of `text`. */
function utf8Length(text: string): number {
  let bytes = 0
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i)
    if (c < 0x80) bytes += 1
    else if (c < 0x800) bytes += 2
    else if (c >= 0xd800 && c < 0xdc00 && i + 1 < text.length) { bytes += 4; i++ }
    else bytes += 3
  }
  return bytes
}

/**
 * The fewest bytes the text this transaction inserted (and did not delete
 * again) can take in any encoding of a later state. Only items in
 * `[beforeState, afterState)` count, so a peer lacking those clocks lacks
 * all of it. Counted in full: a bound that starts low (CJK text is short in
 * UTF-16 but 3 bytes per character) would cost a whole encode at the first
 * deletion (#652 N2).
 */
function insertedBytes(tr: Y.Transaction): number {
  let bytes = 0
  for (const [client, end] of tr.afterState) {
    const start = tr.beforeState.get(client) ?? 0
    const structs = tr.doc.store.clients.get(client)
    if (end <= start || !structs) continue
    for (let i = Y.findIndexSS(structs, start); i < structs.length; i++) {
      const s = structs[i]
      if (!(s instanceof Y.Item) || s.deleted || !(s.content instanceof Y.ContentString)) continue
      // A new item can merge into an older one: count only the new part.
      bytes += utf8Length(s.content.str.slice(Math.max(0, start - s.id.clock)))
    }
  }
  return bytes
}

type TabMsg = (
  | {
      t: 'u'
      u: Uint8Array
      /** The sending tab was offline: it can't send this itself yet. */
      off?: boolean
      /** The sending tab's writer id, its own-edit number for this update and its newest durable one. */
      from: number
      n: number
      ok?: number
      /** An answer to a joining tab: may also hold the server's changes. */
      join?: boolean
    }
  | { t: 'sv'; sv: Uint8Array; from: number }
  | { t: 'ok'; from: number; n: number }
  | { t: 'no'; from: number; n: number; code: 'too_large' | 'error' }
) & {
  /** The sender's lineage. */
  l?: string | null
}

export class SyncClient {
  readonly status = new StatusBox()
  private readonly doc: Y.Doc
  private readonly transport: Transport
  private readonly gate: ImeGate
  private readonly opts: SyncOptions
  private readonly receipts: Receipts
  private bc: BroadcastChannel | null = null
  private destroyed = false
  private synced = false
  private connection = 0
  private own = 0
  private remote = 0
  private recovery = new Map<string, number>()
  private siblingTimer: ReturnType<typeof setTimeout> | null = null
  private checkTimer: ReturnType<typeof setTimeout> | null = null
  private baseTimer: ReturnType<typeof setTimeout> | null = null
  private baseFirst = 0
  private wanted = false
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private retries = 0
  private retryRefusal = false
  /** Sync step 1s from the room on this connection: the first is the join, later ones ask for a resend. */
  private asked = 0
  /**
   * The room's state vector from a resend request it may still be waiting
   * on. Meanwhile the room holds this connection's new save checks; only a
   * sync step 2 can end the wait (crates/notes sync.rs). Checks queued
   * before the gap may still be answered, so only an answer to a check sent
   * after a step 2 shows the wait is over (`owedCheck`).
   */
  private owed: Uint8Array | null = null
  /** A step 2 went out since the resend request. */
  private owedStep2 = false
  /** The first check sent after that step 2: its answer (or a later one) ends the wait. */
  private owedCheck: number | null = null
  /** Something the owed resend lacked became available: send a step 2 with the next check. */
  private resendOwed = false
  /** The room's newest state vector on this connection (from its step 1s). */
  private serverVector: Uint8Array | null = null
  /**
   * After a state did not fit: a lower bound on the bytes still needed for
   * the items in `[from, to)`, lowered by every deletion since. While it is
   * over the limit, and a peer lacks all of those items, the state can't
   * fit, so it is not encoded again (a 64 MiB encode takes most of a second).
   */
  private oversize: { bytes: number; from: Map<number, number>; to: Map<number, number> } | null = null
  private onTransaction = (tr: Y.Transaction) => {
    const o = this.oversize
    if (!o) return
    for (const ranges of tr.deleteSet.clients.values()) for (const r of ranges) o.bytes -= r.len * FREED_PER_UNIT
  }

  constructor(opts: SyncOptions) {
    this.opts = opts
    this.doc = opts.doc
    this.transport = opts.transport
    this.gate = opts.gate
    this.receipts = new Receipts(opts.pendingSince, opts.heldSince, opts.refused, () => this.receiptProgress())
    for (const r of opts.recovery ?? []) this.recovery.set(r.id, this.receipts.observe('recovery', 0, true, r.at, undefined, false))
    this.doc.on('afterTransaction', this.onTransaction)
    this.doc.on('update', this.onDocUpdate)
    this.transport.onOpen(this.onOpen)
    this.transport.onClose(this.onClose)
    this.transport.onMessage(this.onMessage)
    this.transport.onError?.(this.onError)
    opts.awareness?.on('update', this.onAwarenessUpdate)
    this.openTabs()
    this.publish()
  }

  private lineage(): string | null { return this.opts.lineage?.() ?? null }
  private openTabs() {
    if (this.bc || this.destroyed || !this.opts.channel || typeof BroadcastChannel === 'undefined') return
    if (this.opts.lineage && !this.lineage()) return
    this.bc = new BroadcastChannel(this.opts.channel)
    this.bc.onmessage = (ev) => this.onTab(ev.data)
    this.post({ t: 'sv', sv: Y.encodeStateVector(this.doc), from: this.doc.clientID })
  }

  connect(): void { this.wanted = true; if (!this.retryTimer) this.transport.connect() }
  disconnect(): void {
    this.wanted = false
    if (this.retryTimer) clearTimeout(this.retryTimer)
    this.retryTimer = null
    this.transport.disconnect(); this.publish()
  }
  /** A sibling refusal may only delay confirmation, never grant it. */
  refuse(code: 'too_large' | 'error', recheck = false): void {
    this.receipts.refuse(code)
    if (recheck) this.retryChangedRefusal()
    this.publish()
  }
  get isSynced(): boolean { return this.synced }
  get isConfirmed(): boolean { return this.receipts.confirmed }
  get pendingSince(): number | null { return this.receipts.pendingSince }
  get heldSince(): number | null { return this.receipts.heldSince }

  /** Only the persisted, canonical replay delta releases an outside-document obligation.
   * Even an already-applied/no-op delta needs a subsequent receipt on this connection. */
  completeRecovery(id: string, update: Uint8Array): void {
    const guard = this.recovery.get(id)
    if (guard == null || this.destroyed) return
    Y.applyUpdate(this.doc, update)
    this.receipts.applied(guard)
    if (this.send(encodeUpdate(update))) this.sentAlone(guard)
    this.recovery.delete(id)
    this.scheduleCheck()
    this.publish()
  }

  destroy(): void {
    if (this.destroyed) return
    this.gate.end()
    this.destroyed = true
    this.wanted = false
    if (this.retryTimer) clearTimeout(this.retryTimer)
    this.doc.off('afterTransaction', this.onTransaction)
    this.opts.awareness?.setLocalState(null)
    this.opts.awareness?.off('update', this.onAwarenessUpdate)
    this.doc.off('update', this.onDocUpdate)
    this.transport.disconnect()
    this.bc?.close()
    if (this.checkTimer) clearTimeout(this.checkTimer)
    if (this.baseTimer) clearTimeout(this.baseTimer)
    if (this.siblingTimer) clearTimeout(this.siblingTimer)
    this.receipts.destroy()
  }

  private onDocUpdate = (update: Uint8Array, origin: unknown, _doc: Y.Doc, tr: Y.Transaction) => {
    // Remote packets were registered when received, before the IME gate.
    if (origin === this || origin === TAB_ORIGIN) return
    const n = ++this.own
    const id = this.receipts.observe(String(this.doc.clientID), n, true, Date.now(), update)
    this.receipts.proof(id, transactionProof(tr))
    this.retryChangedRefusal()
    const packet = encodeUpdate(update)
    if (packet.length > MAX_ROOM_MESSAGE) {
      // A too-large paste: remember that every later state holds its text,
      // so typing next to it never re-encodes the whole note.
      const bytes = insertedBytes(tr)
      if (bytes > MAX_ROOM_MESSAGE) this.oversize = { bytes, from: tr.beforeState, to: tr.afterState }
    }
    if (this.send(packet)) this.sentAlone(id)
    this.post({ t: 'u', u: update, off: !this.transport.connected, from: this.doc.clientID, n })
    this.scheduleCheck()
    this.publish()
  }

  private send(data: Uint8Array): boolean {
    if (data.length > MAX_ROOM_MESSAGE) {
      this.receipts.refuse('too_large')
      this.publish()
      return false
    }
    if (!this.transport.connected) return false
    this.transport.send(data)
    return true
  }

  private currentStatePacket(stateVector?: Uint8Array): Uint8Array {
    // Always a sync step 2, even unasked: after a gap the room holds this
    // connection's save checks until a step 2 resends what it lacks, and
    // releases them only then (crates/notes room/sync.rs `resynced`).
    const encode = (doc: Y.Doc) => encodeStep2(doc, stateVector ?? EMPTY_STATE_VECTOR)
    let packet = encode(this.doc)
    if (packet.length > MAX_ROOM_MESSAGE) {
      // Undo retains deleted content in the live document. Garbage-collect a
      // separate wire copy, preserving edit identities and deletions without
      // changing what the person can undo. A still-oversized result is refused.
      const wire = new Y.Doc()
      try {
        Y.applyUpdate(wire, Y.encodeStateAsUpdate(this.doc))
        packet = encode(wire)
      } finally { wire.destroy() }
    }
    this.oversize = packet.length > MAX_ROOM_MESSAGE ? {
      bytes: packet.length,
      from: stateVector ? Y.decodeStateVector(stateVector) : new Map(),
      to: Y.decodeStateVector(Y.encodeStateVector(this.doc)),
    } : null
    return packet
  }

  /**
   * The complete state, or its diff for a peer holding `target`, is known
   * not to fit: its text alone is too big, or an earlier encoding was too
   * big and too little has been deleted since.
   */
  private stillTooLarge(target?: Map<number, number>): boolean {
    // Every UTF-16 unit of the text takes at least one byte on the wire.
    if (!target && this.doc.getText('md').length > MAX_ROOM_MESSAGE) return true
    const o = this.oversize
    if (!o || o.bytes <= MAX_ROOM_MESSAGE) return false
    // A peer that already holds some of those items may need less.
    if (target) for (const [client, end] of o.to) {
      const from = o.from.get(client) ?? 0
      if (end > from && (target.get(client) ?? 0) > from) return false
    }
    return true
  }

  private armSiblings() {
    if (this.siblingTimer || !this.receipts.waiting.length) return
    this.siblingTimer = setTimeout(() => {
      this.siblingTimer = null
      if (this.transport.connected) {
        for (const e of this.receipts.waiting) {
          // Allow at least a minute per MiB on a slow uplink. Server echoes
          // normally cover siblings without uploading the same paste again.
          const wait = Math.max(SIBLING_WAIT_MS, e.update.length / (1024 * 1024) * 60_000)
          if (Date.now() - e.at < wait) continue
          if (this.send(encodeUpdate(e.update))) this.sentAlone(e.id)
        }
        this.scheduleCheck()
      }
      this.armSiblings()
      this.publish()
    }, SIBLING_WAIT_MS)
  }

  private scheduleCheck() {
    if (this.checkTimer || !this.transport.connected) return
    this.checkTimer = setTimeout(() => { this.checkTimer = null; this.sendCheck() }, this.opts.checkDelayMs ?? 150)
  }
  /**
   * An edit's own packet went out. While the room waits for a resend it may
   * refuse any update without saying so again: only a sync step 2 holding
   * the edit counts. sendCheck sends one before the check, built from
   * applied state, so an edit still behind IME travels again once applied.
   */
  private sentAlone(id: number) {
    if (!this.owed) { this.receipts.submitted(id); return }
    this.resendOwed = true
    if (this.receipts.isIntegrated(id)) this.receipts.submitted(id)
    else this.receipts.markResend(id)
  }
  /**
   * A packet held behind IME missed what was sent meanwhile: a complete
   * retry after a refusal, or the sync of a newer connection (a pre-crash
   * packet is retained data, never proof about the new server). It goes
   * again on its own when it fits, instead of another full copy. One too
   * big for that waits for the complete state: deletions queued after it
   * may have removed it.
   */
  private resendHeld(id: number, update: Uint8Array, connection: number) {
    if (this.owed) {
      // The room is waiting for a resend: only a step 2 can satisfy it.
      this.resendOwed = true
      this.scheduleCheck()
      return
    }
    const old = connection !== this.connection
    // Also an edit a resend request unsubmitted while it waited (`unsubmitHeld`).
    if (!old && !this.receipts.isRefused(id) && !this.receipts.needsResend(id)) return
    const packet = encodeUpdate(update)
    if (packet.length > MAX_ROOM_MESSAGE) {
      if (old) this.retryState()
      return
    }
    if (!this.send(packet)) return
    this.receipts.resubmitted(id)
    this.scheduleCheck()
  }
  /** Edits a refusal holds back go again with the next complete submission. */
  private retryChangedRefusal() {
    if (this.receipts.awaitingRetry) this.retryState()
  }
  /**
   * Asks the next check to send the complete current state once. Skipped
   * while that state can't fit: a deletion is needed first. Edits made
   * after it is sent travel as ordinary updates.
   */
  private retryState() {
    if (this.stillTooLarge()) return
    this.retryRefusal = true
    if (this.wanted && !this.transport.connected && !this.retryTimer) this.transport.connect()
    this.scheduleCheck()
  }
  private scheduleBaseCheck() {
    if (!this.transport.connected) return
    const now = Date.now()
    if (this.baseTimer) clearTimeout(this.baseTimer)
    else this.baseFirst = now
    const delay = Math.min(this.opts.baseCheckDelayMs ?? 300, this.baseFirst + BASE_CHECK_MAX_MS - now)
    this.baseTimer = setTimeout(() => {
      this.baseTimer = null
      if (this.receipts.pending && !this.receipts.coveredByCheck) this.sendCheck()
    }, Math.max(0, delay))
  }
  private sendCheck() {
    if (!this.transport.connected) return
    // A receipt may have ended the wait meanwhile; the held edits still need
    // sending, as a diff from what the room last said it had.
    const owed = this.owed ?? this.serverVector
    if (this.resendOwed && owed && !this.retryRefusal) {
      this.resendOwed = false
      if (!this.stillTooLarge(Y.decodeStateVector(owed))) {
        const through = this.receipts.position
        const proof = Y.snapshot(this.doc)
        if (this.send(this.currentStatePacket(owed))) this.submittedState(through, proof)
      }
    }
    if (this.retryRefusal && !this.stillTooLarge()) {
      this.retryRefusal = false
      // Retry the complete current state, including deletions, rather than
      // the original paste. Undo-retained content is compacted on the wire.
      const through = this.receipts.position
      const proof = Y.snapshot(this.doc)
      const packet = this.currentStatePacket()
      if (decodeMessage(packet).kind !== 'step2' || !this.send(packet)) return
      this.submittedState(through, proof)
    }
    const id = this.receipts.check()
    if (this.owed && this.owedStep2 && this.owedCheck == null) this.owedCheck = id
    this.transport.send(encodeSaveCheck(id))
  }

  /**
   * The complete state held at `through` is on its way. Its receipt proves
   * the server holds everything in `proof`, a snapshot taken before encoding.
   */
  private submittedState(through: number, proof: Y.Snapshot) {
    if (this.owed) this.owedStep2 = true
    this.receipts.submittedThrough(through, true)
    const id = this.receipts.observe('sync', ++this.remote, false)
    this.receipts.proof(id, proof)
    this.receipts.submitted(id)
  }

  private onError = (code: string) => {
    if (permanentRefusal(code)) {
      this.receipts.refuse(code)
      // The rejected packet can predate a deletion already made here. Rejoin
      // with the current state; only its new receipt can clear this refusal.
      if (code === 'too_large') this.retry()
      this.publish()
      return
    }
    this.retry()
  }
  private retry() {
    if (this.destroyed || !this.wanted || this.retryTimer) return
    this.transport.disconnect()
    this.synced = false
    const delay = [100, 250, 500, 1000, 2000, 5000][Math.min(this.retries++, 5)]!
    this.retryTimer = setTimeout(() => {
      this.retryTimer = null
      if (this.wanted && !this.destroyed) this.transport.connect()
    }, delay)
    this.publish()
  }

  private receiptProgress() {
    if (this.destroyed) return
    this.retries = 0
    if (this.receipts.pending) this.scheduleBaseCheck()
    this.publish()
    if (this.isConfirmed && this.synced) this.opts.onConfirmed?.()
  }

  /** Local presence changed: tell the room (only this client's own entry travels; remote ones were applied from it). */
  private onAwarenessUpdate = ({ added, updated, removed }: { added: number[]; updated: number[]; removed: number[] }, origin: unknown) => {
    const a = this.opts.awareness
    if (!a || origin === 'remote' || !this.transport.connected) return
    const own = [...added, ...updated, ...removed].filter((id) => id === this.doc.clientID)
    if (own.length) this.transport.send(encodeAwareness(awarenessProtocol.encodeAwarenessUpdate(a, own)))
  }

  private onOpen = () => {
    this.connection++
    this.asked = 0
    this.owed = this.serverVector = this.owedCheck = null
    this.owedStep2 = this.resendOwed = false
    this.synced = false
    this.receipts.beginConnection()
    this.transport.send(encodeStep1(this.doc))
    const a = this.opts.awareness
    if (a?.getLocalState()) this.transport.send(encodeAwareness(awarenessProtocol.encodeAwarenessUpdate(a, [this.doc.clientID])))
    this.publish()
  }
  private onClose = (code: number, reason: string) => {
    this.connection++
    this.owed = this.owedCheck = null
    this.owedStep2 = this.resendOwed = false
    this.receipts.beginConnection()
    this.synced = false
    if (this.checkTimer) clearTimeout(this.checkTimer)
    if (this.baseTimer) clearTimeout(this.baseTimer)
    this.checkTimer = this.baseTimer = null
    const a = this.opts.awareness
    if (a) awarenessProtocol.removeAwarenessStates(a, [...a.getStates().keys()].filter((id) => id !== this.doc.clientID), 'remote')
    this.publish()
    if (code === CLOSE_LINEAGE && !this.destroyed) this.opts.onLineageMismatch?.(reason)
    if (code === CLOSE_REVOKED && !this.destroyed) {
      this.wanted = false
      if (this.retryTimer) clearTimeout(this.retryTimer)
      this.retryTimer = null
      this.opts.onAccessRevoked?.()
    }
  }
  private onMessage = (data: Uint8Array) => {
    const msg = decodeMessage(data)
    switch (msg.kind) {
      case 'step1': {
        // After the join, a step 1 asks for a resend: the room holds checks until a step 2 fills it.
        if (++this.asked > 1) {
          this.owed = msg.stateVector
          this.owedStep2 = false
          this.owedCheck = null
          this.receipts.unsubmitHeld()
        }
        this.serverVector = msg.stateVector
        // A state known not to fit stays refused until a deletion; the
        // check below still covers what this connection can confirm.
        if (this.stillTooLarge(Y.decodeStateVector(msg.stateVector))) { this.sendCheck(); break }
        const through = this.receipts.position
        const proof = Y.snapshot(this.doc)
        const packet = this.currentStatePacket(msg.stateVector)
        if (this.send(packet)) this.submittedState(through, proof)
        this.sendCheck()
        break
      }
      case 'step2':
      case 'update': {
        // Holding bytes behind IME already observes state for the replay base.
        // The first snapshot also delays Saved until its covering receipt.
        const connection = this.connection
        const id = this.receipts.observe('server', ++this.remote, false, Date.now(), msg.update, false, msg.kind === 'step2' && !this.synced)
        this.receipts.serverUpdate(msg.update)
        this.receipts.submitted(id)
        this.scheduleBaseCheck()
        this.publish()
        this.gate.run(() => {
          if (this.destroyed) return
          Y.applyUpdate(this.doc, msg.update, this)
          this.receipts.applied(id)
          this.resendHeld(id, msg.update, connection)
          this.retryChangedRefusal()
          if (connection === this.connection && msg.kind === 'step2' && !this.synced && this.transport.connected) {
            this.synced = true
            this.openTabs()
            this.opts.onSynced?.()
          }
          this.scheduleBaseCheck()
          this.publish()
          if (this.isConfirmed && this.synced) this.opts.onConfirmed?.()
        })
        break
      }
      case 'awareness':
        if (this.opts.awareness) awarenessProtocol.applyAwarenessUpdate(this.opts.awareness, msg.update, 'remote')
        break
      case 'refused':
        this.onError(msg.code)
        break
      case 'retry':
        this.retry()
        break
      case 'saved':
        // Only an answer to a check sent after a step 2 shows the room stopped waiting.
        if (this.owedCheck != null && msg.id >= this.owedCheck) {
          this.owed = null
          this.owedCheck = null
        }
        if (!this.receipts.receipt(msg.id)) return
        this.receiptProgress()
        break
    }
  }

  private post(m: TabMsg) { this.bc?.postMessage({ ...m, l: this.lineage() }) }
  private onTab(value: unknown) {
    if (!value || typeof value !== 'object') return
    const m = value as TabMsg
    if ((m.l ?? null) !== this.lineage()) return
    // Sibling receipts are hints only. They cannot mutate the receipt ledger.
    if (m.t === 'ok') return
    if (m.t === 'no') {
      if (!validSequence(m.from) || !validSequence(m.n) || !['too_large', 'error'].includes(m.code)) return
      this.refuse(m.code, true)
      return
    }
    if (m.t === 'u') {
      if (!(m.u instanceof Uint8Array)) return
      if (!validSequence(m.from) || !validSequence(m.n)) return
      if (m.ok != null && !validSequence(m.ok)) return
      if (m.off != null && typeof m.off !== 'boolean') return
      if (m.join != null && typeof m.join !== 'boolean') return
      let proof: CoverageProof
      try {
        proof = updateProof(m.u)
        // Duplicate hints add no observed state. Never invent pending edits
        // merely because another tab answered a state-vector request.
        if (Y.snapshotContainsUpdate(Y.snapshot(this.doc), m.u)) return
      } catch { return }
      const connection = this.connection
      const id = this.receipts.observeSibling(String(m.from), m.n, m.u, Date.now(), false, proof)
      if (m.off || m.u.length >= MAX_ROOM_MESSAGE - 16) {
        const packet = encodeUpdate(m.u)
        if ((m.off || packet.length > MAX_ROOM_MESSAGE) && this.send(packet)) this.sentAlone(id)
      }
      this.scheduleCheck()
      this.armSiblings()
      this.publish()
      this.gate.run(() => {
        if (this.destroyed) return
        Y.applyUpdate(this.doc, m.u, TAB_ORIGIN)
        this.receipts.applied(id)
        this.resendHeld(id, m.u, connection)
        this.retryChangedRefusal()
        this.publish()
        if (this.isConfirmed && this.synced) this.opts.onConfirmed?.()
      })
    } else if (m.t === 'sv' && validSequence(m.from) && m.from !== this.doc.clientID && m.sv instanceof Uint8Array) {
      try {
        const u = Y.encodeStateAsUpdate(this.doc, m.sv)
        this.post({ t: 'u', u, off: !this.transport.connected, from: this.doc.clientID, n: this.own, join: true })
        const code = this.receipts.refusal
        if (code) this.post({ t: 'no', from: this.doc.clientID, n: this.own, code })
      } catch { /* A malformed state vector cannot grant confirmation. */ }
    }
  }

  private lastRefusal: 'too_large' | 'error' | null | undefined = undefined
  private lastPending: number | null | undefined = undefined
  private lastHeld: number | null | undefined = undefined
  private publish() {
    const refusal = this.receipts.refusal
    if (refusal !== this.lastRefusal) {
      this.lastRefusal = refusal
      this.opts.onRefusalChange?.(refusal)
      if (refusal) this.post({ t: 'no', from: this.doc.clientID, n: this.own, code: refusal })
    }
    const status = this.receipts.status(this.transport.connected)
    this.status.set(status)
    const since = this.pendingSince
    if (since !== this.lastPending) { this.lastPending = since; this.opts.onPendingChange?.(since) }
    const held = this.heldSince
    if (held !== this.lastHeld) { this.lastHeld = held; this.opts.onHeldChange?.(held) }
  }
}

function validSequence(n: unknown): n is number { return Number.isSafeInteger(n) && (n as number) >= 0 }
