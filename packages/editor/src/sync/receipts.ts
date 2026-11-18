/**
 * The sole authority for Saved and the safe replay base (ADR 0020).
 *
 * Every observed edit has an origin and sequence. A check captures only
 * edits submitted on this connection or independently observed in server
 * packets BEFORE the check. Only that check's server receipt retires them.
 * Sibling messages never retire edits. A refusal survives until a fresh full
 * submission has its own covering receipt. Saved covers this device's edits;
 * the replay base additionally requires receipts for every observed peer edit.
 */
import * as Y from 'yjs'
import type { SaveStatus } from './status'
import { Coverage, type CoverageProof, updateProof } from './coverage'
import { PendingTimes, type PendingTime } from './pending-times'

/** An observed edit and the evidence needed to retire it; its timestamp stays indexed until then. */
type Edit = PendingTime & {
  origin: string
  sequence: number
  local: boolean
  required: boolean
  submitted: boolean
  integrated: boolean
  refused: boolean
  /** Sent, but a later resend request means the room may have refused it (`unsubmitHeld`). */
  resend?: boolean
  update?: Uint8Array
  /** What the server holds once this edit's receipt arrives (ids and deletes, no text). */
  proof?: CoverageProof | Y.Snapshot
}

/** Longest one receipt-drain step runs before yielding to input and paint. */
const DRAIN_BUDGET_MS = 4

// Memberships on one live socket can overlap while old replies drain. IDs
// must never restart when a note is closed and reopened in this page.
let checkSequence = 0

/** Owns receipt coverage and indexes pending timestamps without adding work after a drain yields. */
export class Receipts {
  private edits = new Map<number, Edit>()
  private next = 0
  private checks = new Map<number, { through: number; ids: number[] }>()
  private unknown = true
  private failure: string | null = null
  private failureThrough = 0
  /** What the server is known to hold on this connection: server packets plus receipted proofs. */
  private server = new Coverage()
  private local = 0
  private localTimes = new PendingTimes()
  private heldTimes = new PendingTimes()
  private work: Array<{ ids: number[]; at: number }> = []
  /**
   * Drain steps run as MessageChannel tasks: not clamped or throttled like
   * chained timers, so a hidden tab clears a long offline backlog quickly.
   */
  private workChannel: MessageChannel | null = null
  private scheduled = false

  constructor(pending: number | null = null, held: number | null = null, refused: string | null = null, private readonly changed: () => void = () => {}) {
    if (pending != null) this.observe('carried', 0, true, pending)
    if (held != null) this.observe('held', 0, false, held)
    if (refused) this.refuse(refused)
  }

  /** Records an obligation and its exact time; later status reads never walk these edits. */
  observe(origin: string, sequence: number, local: boolean, at = Date.now(), update?: Uint8Array, integrated = true, required = local || origin === 'sync'): number {
    const id = ++this.next
    const edit: Edit = { origin, sequence, local, required, at, timeSlot: -1, submitted: false, refused: false, update, integrated }
    this.edits.set(id, edit)
    ;(local ? this.localTimes : this.heldTimes).add(edit)
    if (required) this.local++
    return id
  }

  submitted(id: number): void {
    const edit = this.edits.get(id)
    if (edit && !edit.refused) { edit.submitted = true; delete edit.update; delete edit.resend }
  }

  /** A successful sync step 2 submitted all state held when it was encoded. */
  submittedThrough(through: number, retryRefused = false): void {
    for (const [id, e] of this.edits) if (id <= through && e.integrated) {
      // The caller records one proof for the complete sync packet. Replaying
      // every original insertion would also reconstruct content since deleted.
      if (retryRefused) { e.refused = false; delete e.proof }
      this.submitted(id)
    }
  }
  applied(id: number): void { const e = this.edits.get(id); if (e) e.integrated = true }
  /** Records what a successful receipt for this edit proves the server holds. */
  proof(id: number, proof: CoverageProof | Y.Snapshot): void {
    const e = this.edits.get(id)
    if (e && !e.refused) e.proof = proof
  }
  isRefused(id: number): boolean { return this.edits.get(id)?.refused === true }
  isIntegrated(id: number): boolean { return this.edits.get(id)?.integrated === true }
  /** Sent while the room may refuse it unannounced, and not applied here: it must travel again once applied. */
  markResend(id: number): void { const e = this.edits.get(id); if (e && !e.refused) e.resend = true }
  /** Unsubmitted by a resend request while behind IME: it must travel again. */
  needsResend(id: number): boolean { return this.edits.get(id)?.resend === true }
  /**
   * The room asked for a resend: something sent may have been refused for a
   * gap. A step 2 answer can only hold what is applied here, so edits still
   * behind IME must travel again once applied.
   */
  unsubmitHeld(): void {
    for (const e of this.edits.values()) if (!e.integrated && e.submitted) { e.submitted = false; e.resend = true }
  }
  /** A refused edit was sent again on its own; a later receipt can retire it. */
  resubmitted(id: number): void {
    const e = this.edits.get(id)
    if (e) { e.refused = false; this.submitted(id) }
  }
  /** Some applied edit was refused and has not been resubmitted since. */
  get awaitingRetry(): boolean {
    if (!this.failure) return false
    for (const e of this.edits.values()) if (e.refused && e.integrated) return true
    return false
  }
  get position(): number { return this.next }
  get refusal(): 'too_large' | 'error' | null { return this.failure as 'too_large' | 'error' | null }
  get refused(): boolean { return this.failure != null }

  /** Server packets prove coverage, including deletes, but do not prove durability. */
  serverUpdate(update: Uint8Array): void {
    this.server.update(update)
    const proof = this.server.snapshot
    for (const [id, e] of this.edits) {
      if (e.update && Y.snapshotContainsUpdate(proof, e.update)) this.submitted(id)
    }
  }

  /** New sibling state can arrive after its server echo; still require a later check. */
  observeSibling(origin: string, sequence: number, update: Uint8Array, at = Date.now(), integrated = true, proof: CoverageProof = updateProof(update)): number {
    const id = this.observe(origin, sequence, true, at, update, integrated)
    this.proof(id, proof)
    if (Y.snapshotContainsUpdate(this.server.snapshot, update)) this.submitted(id)
    return id
  }

  get waiting(): Array<{ id: number; update: Uint8Array; at: number }> {
    return [...this.edits].flatMap(([id, e]) => e.update && !e.refused ? [{ id, update: e.update, at: e.at }] : [])
  }

  refuse(code: string): void {
    this.failure = code === 'too_large' ? 'too_large' : (this.failure ?? 'error')
    if (!this.edits.size) this.observe('refused', 0, true)
    this.failureThrough = this.next
    for (const e of this.edits.values()) { e.refused = true; delete e.update; delete e.proof }
    // Replies to checks sent before the refusal cannot cover a later retry.
    this.checks.clear()
    this.cancelWork()
  }

  beginConnection(): void {
    this.cancelWork()
    this.checks.clear()
    this.server = new Coverage()
    // Submitted but unreceipted edits must travel again on the new connection.
    for (const e of this.edits.values()) e.submitted = false
  }

  check(): number {
    const id = ++checkSequence
    this.checks.set(id, { through: this.next, ids: [...this.edits].filter(([, e]) => e.submitted && !e.refused).map(([id]) => id) })
    if (this.checks.size > 64) this.checks.delete(this.checks.keys().next().value!)
    return id
  }

  receipt(id: number): boolean {
    // Unknown/future ids cannot confirm earlier checks.
    const check = this.checks.get(id)
    if (!check) return false
    for (const key of this.checks.keys()) if (key <= id) this.checks.delete(key)
    this.work.push({ ids: check.ids, at: 0 })
    if (!this.scheduled) this.drain()
    return true
  }

  private cancelWork(): void {
    this.closeChannel()
    this.work = []
  }
  private closeChannel(): void {
    // An open port keeps a worker or Node process alive; close it when idle.
    this.workChannel?.port1.close()
    this.workChannel = null
    this.scheduled = false
  }
  private yieldWork(): void {
    if (!this.workChannel) {
      const channel = new MessageChannel()
      channel.port1.onmessage = () => {
        if (channel !== this.workChannel) return
        this.drain()
        this.changed()
      }
      this.workChannel = channel
    }
    this.scheduled = true
    this.workChannel.port2.postMessage(null)
  }

  /** Long sessions yield between batches; timestamp retirement also stays inside this deadline. */
  private drain(): void {
    this.scheduled = false
    const until = performance.now() + DRAIN_BUDGET_MS
    let count = 0
    while (this.work.length) {
      const job = this.work[0]!
      while (job.at < job.ids.length) {
        const id = job.ids[job.at++]!
        const e = this.edits.get(id)
        if (e && !e.refused) {
          // A sender has no server echo: only its receipt grants this proof.
          // Proofs hold ids and deletes only, so even a whole-note one is cheap.
          if (e.proof) this.server.add(e.proof)
          if (e.required) this.local--
          ;(e.local ? this.localTimes : this.heldTimes).remove(e)
          this.edits.delete(id)
        }
        // Yield only with work left: finishing must not wait for another task.
        const more = job.at < job.ids.length || this.work.length > 1
        if (more && (++count >= 64 || performance.now() >= until)) {
          this.yieldWork()
          return
        }
      }
      this.work.shift()
      this.unknown = false
    }
    this.closeChannel()
    // Later peer typing cannot extend a refusal for an earlier rejected edit.
    // Every obligation present at refusal still needs its fresh receipt.
    let obligation = false
    let proof: Y.Snapshot | undefined
    // One pass avoids allocating two temporary arrays after the bounded work.
    for (const [id, e] of this.edits) {
      if (id <= this.failureThrough) obligation = true
      if (e.update && Y.snapshotContainsUpdate(proof ??= this.server.snapshot, e.update)) this.submitted(id)
    }
    if (!obligation) this.failure = null
  }

  get coveredByCheck(): boolean {
    return [...this.checks.values()].some((c) => c.through === this.next && c.ids.length === this.edits.size)
  }
  get confirmed(): boolean { return !this.unknown && !this.failure && this.edits.size === 0 && this.work.length === 0 }
  get pending(): boolean { return this.unknown || this.edits.size > 0 }
  /** Exact oldest local obligation, readable in constant time between drain tasks. */
  get pendingSince(): number | null { return this.localTimes.oldest }
  /** Exact oldest peer obligation, without scanning the remaining backlog. */
  get heldSince(): number | null { return this.heldTimes.oldest }

  status(connected: boolean): SaveStatus {
    return {
      state: !this.unknown && !this.failure && this.local === 0 ? 'saved' : connected && !this.failure ? 'saving' : 'not-saved',
      since: this.pendingSince,
      connected,
      tooLarge: this.failure === 'too_large',
      error: this.failure && this.failure !== 'too_large' ? 'Could not save' : undefined,
    }
  }

  destroy(): void { this.cancelWork() }
}
