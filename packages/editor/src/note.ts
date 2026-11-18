/**
 * One open note: its shared document, its copy on this device, and its
 * connection to the server.
 *
 * The document holds the note's markdown as a single shared text (`md`).
 * The editor binds to the text; nothing else writes to it. Which lineage
 * the document grew from is learned when the room is joined and kept in
 * the note's metadata on this device, beside the document.
 */

import type { Awareness } from 'y-protocols/awareness'
import * as Y from 'yjs'
import { LocalCopy } from './offline/local-copy'
import { type MetaStore, type NoteMeta, type RecoveryIntent, idbMetaStore, memoryMetaStore } from './offline/meta'
import { SyncClient, TAB_ORIGIN } from './sync/client'
import { ImeGate } from './sync/gate'
import { StatusBox } from './sync/status'
import type { Transport } from './sync/transport'

export const TEXT_KEY = 'md'
/** Origin for changes replayed after the server's copy was replaced. */
export const REPLAY_ORIGIN = Symbol('calternal-replay')

export interface OpenNoteOptions {
  id: string
  /**
   * Makes the note's sync transport, usually a room on the live socket
   * (`liveSocket.room('notes/<id>', { lineage, onLineage })`). `lineage`
   * tells the server which copy this device holds, so an older copy is
   * never merged; `onLineage` hands back the room's lineage on join.
   */
  transport: (lineage: () => string | null, onLineage: (lineage: string) => void) => Transport
  /** Keep a copy on this device (IndexedDB). Defaults to true when available. */
  offline?: boolean
  meta?: MetaStore
  /** Local changes that couldn't be merged safely; the app keeps them as a copy. */
  onConflictCopy?: (text: string) => void
  checkDelayMs?: number
  /** Presence for this note's room: made for each document the session holds (a replaced document gets a new one). */
  awareness?: (doc: Y.Doc) => Awareness
  /** The room ended the membership because access was lost: the session is frozen (no more edits) and this is told once, so the host can keep what was written. */
  onAccessRevoked?: () => void
}

/** Every open note editor in this page (shared-copy.ts reads it). */
export const sessions = new Set<NoteSession>()
/**
 * Closed notes whose client still waits for a collaborator change's receipt
 * (`NoteSession.linger`), by note id: stops it.
 */
export const lingering = new Map<string, () => void>()
/** Longest a closed note's client waits for such a receipt. */
const LINGER_MS = 30_000
/** Set once this page is leaving (signing out): no note opens or writes its copy any more. */
let stopped = false
let frozen = false
const forgetPrefix = 'calternal-note-'

if (typeof window !== 'undefined') {
  addEventListener('storage', (event) => {
    if (!event.key?.startsWith(forgetPrefix) || !event.key.endsWith(':forget')) return
    const id = event.key.slice(forgetPrefix.length, -':forget'.length)
    // The forget code loads with sharing (shared-copy.ts), not with every editor.
    if (id) void import('./shared-copy').then((m) => m.forgetNote(id, false))
  })
}

/** Pause local editing while sign-out checks for unsaved work; sync keeps draining. */
export function freezeNotes(): void {
  if (frozen) return
  frozen = true
  for (const s of sessions) s.notifyReadOnly()
}

/** Cancelling sign-out allows editing again, unless this page already stopped. */
export function resumeNotes(): void {
  if (stopped || !frozen) return
  frozen = false
  for (const s of sessions) s.notifyReadOnly()
}

/** Stops edits to one shared note while its pending local work is being rescued. */
export function freezeSharedNote(id: string): void {
  for (const s of sessions) {
    if (s.id === id) s.freezeForShareEnd()
  }
}

/**
 * This page is about to be replaced because the device signs out. Closes
 * every open note now (sync, local copy, timers) and refuses to open more,
 * so nothing in this page writes a note's copy again while the next page
 * deletes them: a reload replaces the page only some time later.
 */
export function stopNotes(): void {
  stopped = true
  freezeNotes()
  for (const s of [...sessions]) s.destroy()
  for (const stop of [...lingering.values()]) stop()
}

/** How long a collaborator's change may wait for its receipt before it is written down as held. */
const HELD_RECORD_MS = 2000
/**
 * Whether open notes have unsaved work. A sign-out cutoff waits for every held change
 * already observed at that moment, without waiting forever on later collaborator typing.
 * Ordinary polling only warns once a held change is two seconds old.
 */
export const pendingNotes = (cutoff?: number) => [...sessions].some((s) => s.status.get().since != null ||
  (s.client?.heldSince != null && (cutoff == null
    ? Date.now() - s.client.heldSince >= HELD_RECORD_MS
    : s.client.heldSince <= cutoff)))

type Replaced = (doc: Y.Doc) => void

export class NoteSession {
  readonly id: string
  readonly gate = new ImeGate()
  /** Save status that survives the document being replaced. */
  readonly status = new StatusBox()
  doc!: Y.Doc
  /** This document's presence, when the host asked for it (`OpenNoteOptions.awareness`). */
  awareness: Awareness | null = null
  client!: SyncClient
  private persistence: LocalCopy | null = null
  private meta!: MetaStore
  private stored: NoteMeta = { lineage: null, base: null, pendingSince: null }
  private replacedFns = new Set<Replaced>()
  private unsubStatus: (() => void) | null = null
  private syncedFns = new Set<() => void>()
  private readOnlyFns = new Set<() => void>()
  private destroyed = false
  private shareEnded = false
  private recovery: RecoveryIntent[] = []
  private recovering = false
  private replaying: SyncClient | null = null
  private completing: SyncClient | null = null
  private replacing = false
  private readonly refusalKey: string
  private storedRefusal(): 'too_large' | 'error' | null {
    try {
      const code = localStorage.getItem(this.refusalKey)
      return code === 'too_large' || code === 'error' ? code : null
    } catch { return null }
  }
  private onStorage = (event: StorageEvent) => {
    if (event.key !== this.refusalKey) return
    const code = this.storedRefusal()
    if (code) this.client?.refuse(code, true)
  }

  get readOnly(): boolean { return frozen || stopped || this.destroyed || this.recovering || this.shareEnded }

  onReadOnly(fn: () => void): () => void {
    this.readOnlyFns.add(fn)
    return () => this.readOnlyFns.delete(fn)
  }

  /** Updates editors synchronously before their persistence and sync are closed. */
  notifyReadOnly(): void { this.readOnlyFns.forEach((f) => f()) }

  /** Holds the last local copy still available while recovery is written elsewhere. */
  freezeForShareEnd(): void {
    if (this.destroyed || this.shareEnded) return
    this.shareEnded = true
    this.notifyReadOnly()
    this.gate.end()
  }

  private constructor(private readonly opts: OpenNoteOptions) {
    this.id = opts.id
    this.refusalKey = `calternal-note-${opts.id}:refused`
    if (typeof addEventListener !== 'undefined') addEventListener('storage', this.onStorage)
  }

  /** Opens a note: loads this device's copy first, then connects. */
  static async open(opts: OpenNoteOptions): Promise<NoteSession> {
    if (stopped) throw new Error('this page is signing out')
    const s = new NoteSession(opts)
    sessions.add(s)
    const offline = opts.offline ?? typeof indexedDB !== 'undefined'
    s.meta = opts.meta ?? (offline ? idbMetaStore() : memoryMetaStore())
    try {
      s.stored = await s.meta.get(opts.id)
      s.recovery = (await s.meta.recoveries(opts.id)).filter((r) => !r.done)
      if (s.recovery.length) {
        s.recovering = true
        // A crash may have happened before clearing the old WAL. Keep its
        // lineage metadata until deletion has actually completed.
        if (s.recovery.some((r) => r.lineage === s.stored.lineage)) {
          if (offline) {
            const doc = new Y.Doc()
            const old = await LocalCopy.open(`calternal-note-${s.id}`, doc)
            await old.clear()
            doc.destroy()
          }
          s.stored = { ...s.stored, lineage: null, base: null }
          await s.meta.put(s.id, s.stored)
        }
      }
      await s.start(offline)
      return s
    } catch (e) {
      sessions.delete(s)
      if (typeof removeEventListener !== 'undefined') removeEventListener('storage', s.onStorage)
      throw e
    }
  }

  get text(): Y.Text {
    return this.doc.getText(TEXT_KEY)
  }

  /**
   * The lineage of the copy this device holds: remembered from an earlier
   * visit, or learned when the room was first joined. Null for a fresh
   * copy that hasn't joined yet.
   */
  get lineage(): string | null {
    return this.stored.lineage
  }

  /** Current note text and the last server-confirmed base, only when local edits remain. */
  pendingLocalSnapshot(): { base: string | null; local: string } | null {
    if (this.persistence?.dirtySince == null) return null
    return { base: this.stored.base, local: this.text.toString() }
  }

  /** Stops every writer before its local data is removed by `forgetNote` (shared-copy.ts). */
  async forgetLocalCopy(): Promise<void> {
    if (this.destroyed) return
    this.gate.end()
    this.destroyed = true
    this.notifyReadOnly()
    sessions.delete(this)
    if (typeof removeEventListener !== 'undefined') removeEventListener('storage', this.onStorage)
    if (this.baseTimer) clearTimeout(this.baseTimer)
    if (this.heldTimer) clearTimeout(this.heldTimer)
    this.unsubStatus?.()
    this.persistence?.destroy()
    this.persistence = null
    this.client?.destroy()
    this.text.delete(0, this.text.length)
    await this.meta.remove(this.id)
    try { localStorage.removeItem(this.refusalKey) } catch { /* IndexedDB metadata is removed by the caller. */ }
  }

  /** The room said which lineage it is on; a fresh copy adopts it. */
  private learnLineage(lineage: string) {
    if (this.stored.lineage) return
    this.stored.lineage = lineage
    void this.meta.put(this.id, this.stored)
  }

  /** Calls `fn` when the document is swapped for a new one (lineage change). */
  onReplaced(fn: Replaced): () => void {
    this.replacedFns.add(fn)
    return () => this.replacedFns.delete(fn)
  }

  /** Calls `fn` each time the document has caught up with the server. */
  onSynced(fn: () => void): () => void {
    this.syncedFns.add(fn)
    return () => this.syncedFns.delete(fn)
  }

  connect(): void {
    this.client.connect()
  }

  disconnect(): void {
    this.client.disconnect()
  }

  destroy(): void {
    if (this.destroyed) return
    const client = this.client
    const confirmed = client?.isConfirmed
    this.gate.end()
    if (confirmed && client === this.client && client.isConfirmed && !this.recovering) {
      // A normal close must not cancel the only base write for receipted
      // text, including bytes just drained from IME. Queue this write before
      // the next opener can read metadata and before closing the local copy.
      this.stored.base = this.text.toString()
      void this.meta.put(this.id, this.stored)
    }
    this.destroyed = true
    if (typeof removeEventListener !== 'undefined') removeEventListener('storage', this.onStorage)
    this.notifyReadOnly()
    sessions.delete(this)
    if (this.baseTimer) clearTimeout(this.baseTimer)
    if (this.heldTimer) clearTimeout(this.heldTimer)
    this.unsubStatus?.()
    this.persistence?.destroy()
    // Closing before the delayed marker must still retain the last unconfirmed copy.
    if (client?.heldSince != null) {
      this.stored.heldSince = client.heldSince
      void this.meta.put(this.id, this.stored)
      if (!stopped) return this.linger(client, client.heldSince)
    }
    client?.destroy()
  }

  /**
   * The note closed while a collaborator's change it holds waited for its
   * receipt, usually a moment away. The marker written above keeps that
   * last copy safe if the page goes; the client stays connected (no editor,
   * no local copy) until the receipt arrives, then the marker is cleared.
   * Without this, leaving a note right after a change arrived made every
   * later sign-out warn about it. Gives up after `LINGER_MS`, leaving the
   * marker; reopening the note or signing out stops it at once.
   */
  private linger(client: SyncClient, since: number) {
    lingering.get(this.id)?.()
    let timer: ReturnType<typeof setTimeout> | null = null
    const stop = () => {
      if (timer) clearTimeout(timer)
      timer = null
      if (lingering.get(this.id) === stop) lingering.delete(this.id)
      this.lingerHeld = null
      client.destroy()
    }
    timer = setTimeout(stop, LINGER_MS)
    lingering.set(this.id, stop)
    this.lingerHeld = (now) => {
      if (now != null) return
      void this.meta.clearHeld(this.id, since)
      stop()
    }
  }
  private lingerHeld: ((since: number | null) => void) | null = null

  private async start(offline: boolean) {
    if (stopped || this.destroyed) throw new Error('this page is signing out')
    // This client takes over: its own receipt clears the marker a lingering one left.
    lingering.get(this.id)?.()
    // A fresh document every time: client ids are never reused or stored.
    const doc = new Y.Doc()
    this.doc = doc
    if (offline) {
      if (stopped) throw new Error('this page is signing out')
      this.persistence = await LocalCopy.open(`calternal-note-${this.id}`, doc)
      if (stopped || this.destroyed) {
        this.persistence.destroy()
        throw new Error('this page is signing out')
      }
      // Only edits made here mark the copy unconfirmed, never the server's or another tab's.
      this.persistence.isLocal = (origin) => origin !== this.client && origin !== TAB_ORIGIN
    }
    this.awareness = this.opts.awareness?.(doc) ?? null
    this.client = new SyncClient({
      doc,
      awareness: this.awareness ?? undefined,
      onAccessRevoked: () => { if (this.destroyed) return; this.freezeForShareEnd(); this.opts.onAccessRevoked?.() },
      transport: this.opts.transport(
        () => this.lineage,
        (l) => this.learnLineage(l),
      ),
      gate: this.gate,
      channel: `calternal-note-${this.id}`,
      lineage: () => this.lineage,
      // No invented local marker; the initial server receipt is still required.
      pendingSince: this.stored.pendingSince ?? this.persistence?.dirtySince ?? null,
      recovery: this.recovery,
      checkDelayMs: this.opts.checkDelayMs,
      // A closed note's lingering client only reports held changes (`linger`).
      onPendingChange: (since) => { if (!this.destroyed) void this.onPending(since) },
      refused: this.stored.refused === 'too_large' ? 'too_large' : this.storedRefusal() ?? this.stored.refused ?? null,
      onRefusalChange: (code) => {
        if (this.destroyed) return
        const cleared = this.stored.refused != null && code == null
        this.stored.refused = code
        if (code) {
          try {
            if (localStorage.getItem(this.refusalKey) !== 'too_large') localStorage.setItem(this.refusalKey, code)
          } catch { /* The metadata marker and tab hint still retain refusal. */ }
        } else if (cleared) {
          try { localStorage.removeItem(this.refusalKey) } catch { /* IndexedDB also carries the marker. */ }
          void this.meta.clearRefusal(this.id)
        }
        void this.meta.put(this.id, this.stored)
      },
      heldSince: this.stored.heldSince ?? null,
      onHeldChange: (since) => this.onHeld(since),
      onConfirmed: () => { if (!this.destroyed) void this.onConfirmed() },
      onSynced: () => {
        if (this.destroyed) return
        if (this.recovering) void this.replayRecovery().catch(() => this.client?.refuse('error'))
        this.syncedFns.forEach((f) => f())
      },
      onLineageMismatch: () => { if (!this.destroyed) void this.recover().catch(() => this.client?.refuse('error')) },
    })
    this.unsubStatus?.()
    this.unsubStatus = this.client.status.subscribe((v) =>
      this.status.set(v),
    )
  }

  private heldTimer: ReturnType<typeof setTimeout> | null = null

  /**
   * A collaborator's change this device holds is unconfirmed (or no longer).
   * Its receipt normally follows within a moment, so it is written down
   * only if it is still missing after `HELD_RECORD_MS`: opening a note and
   * leaving at once never makes signing out warn about it.
   */
  private onHeld(since: number | null) {
    if (this.destroyed) return this.lingerHeld?.(since)
    if (this.heldTimer) clearTimeout(this.heldTimer)
    this.heldTimer = null
    const write = (value: number | null) => {
      if ((this.stored.heldSince ?? null) === value) return
      this.stored.heldSince = value
      void this.meta.put(this.id, this.stored)
    }
    if (since == null) return write(null)
    const client = this.client
    this.heldTimer = setTimeout(() => {
      this.heldTimer = null
      if (client === this.client && client.heldSince != null) write(client.heldSince)
    }, HELD_RECORD_MS)
  }

  private async onPending(since: number | null) {
    this.stored.pendingSince = since
    await this.meta.put(this.id, this.stored)
  }

  private baseTimer: ReturnType<typeof setTimeout> | null = null

  /**
   * Everything is confirmed: this text is the new safe base. Collaborators'
   * changes count too, or a replay after a lineage change would insert
   * them a second time. Written at most a few times a second.
   */
  private onConfirmed() {
    if (this.recovering) { void this.finishRecovery().catch(() => this.client?.refuse('error')); return }
    if (this.baseTimer) return
    const client = this.client
    this.baseTimer = setTimeout(() => {
      this.baseTimer = null
      if (client !== this.client || !client.isConfirmed || !client.isSynced) return
      this.persistence?.markClean()
      this.stored.base = this.text.toString()
      this.stored.lineage = this.lineage
      void this.meta.put(this.id, this.stored)
    }, 100)
  }

  /**
   * The server's copy started a new lineage. Keep what this device wrote
   * since the last confirmed base, drop the old document, load the new
   * one, and replay only those changes.
   */
  private async recover() {
    if (this.replacing || this.destroyed) return
    this.replacing = true
    this.recovering = true
    this.notifyReadOnly()
    try {
      // Receipts can cover remote bytes still waiting behind IME. Capture
      // authority before draining: every resulting byte is then receipted,
      // even if the visible-text base's delayed write has not run yet.
      const confirmed = this.client.isConfirmed
      // Queued composition packets belong to the old copy too; include them in its replay.
      this.gate.end()
      // Only needed here, and rarely: loaded now, not with the app (#327).
      const local = this.text.toString()
      // Another replacement during recovery must not discard newly held
      // server/sibling text. With no receipted base, retain it as a durable
      // conflict copy rather than guessing a second text replay.
      const base = this.recovery.length ? null : confirmed && this.client.isConfirmed ? local : this.stored.base
      if (!this.recovery.length || local.trim() !== '') {
        // Identical source captures in concurrent tabs are one obligation.
        // The transaction also writes the sign-out marker before any copy clears.
        const source = { lineage: this.lineage, base, local }
        const list = await this.meta.updateRecoveries(this.id, (list) => {
          if (list.some((r) => r.lineage === source.lineage && r.base === base && r.local === local)) return list
          return [...list, { ...source, at: this.client.pendingSince ?? Date.now(), id: crypto.randomUUID() }]
        })
        this.recovery = list.filter((r) => !r.done || (r.lineage === source.lineage && r.base === base && r.local === local))
      }
      if (this.destroyed) return
      this.client.destroy()
      await this.persistence?.clear()
      this.persistence = null
      if (this.destroyed) return
      this.stored = { ...this.stored, lineage: null, base: null }
      await this.meta.put(this.id, this.stored)
      await this.start(this.opts.offline ?? typeof indexedDB !== 'undefined')
      const doc = this.doc
      this.replacedFns.forEach((f) => f(doc))
      this.client.connect()
    } finally { this.replacing = false }
  }

  private async replayRecovery() {
    if (this.replaying === this.client || this.destroyed || !this.client.isSynced) return
    const client = this.client
    this.replaying = client
    const doc = this.doc
    try {
      const { replay, minimalEdit } = await import('./offline/replay')
      for (const intent of this.recovery) {
        if (client !== this.client || this.destroyed || !client.isSynced) return
        const lineage = this.lineage
        if (!lineage) return
        let plan = intent.plan
        if (!plan || plan.lineage !== lineage) {
          // No safe base means the offered copy is still the only durable
          // recovery. A void UI callback is not evidence that it was saved.
          if (intent.base == null && intent.local.trim() !== '' && intent.local !== this.text.toString()) {
            this.opts.onConflictCopy?.(intent.local)
            continue
          }
          const server = this.text.toString()
          const result = intent.base == null ? { text: server, clean: true } : replay(intent.base, intent.local, server)
          if (!result.clean) { this.opts.onConflictCopy?.(intent.local); continue }
          const fork = new Y.Doc()
          Y.applyUpdate(fork, Y.encodeStateAsUpdate(doc))
          const edit = minimalEdit(server, result.text)
          const text = fork.getText(TEXT_KEY)
          fork.transact(() => {
            if (edit.remove) text.delete(edit.at, edit.remove)
            if (edit.insert) text.insert(edit.at, edit.insert)
          }, REPLAY_ORIGIN)
          // Include prerequisites too: a server crash must not strand the
          // fixed replay writer behind a missing, unreceipted server struct.
          const candidate = { lineage, update: Y.encodeStateAsUpdate(fork), clean: true }
          fork.destroy()
          const list = await this.meta.updateRecoveries(this.id, (list) => list.map((r) => {
            if (r.id !== intent.id || r.plan?.lineage === lineage) return r
            if (r.done) throw new Error('completed recovery belongs to another lineage')
            return { ...r, plan: candidate }
          }))
          plan = list.find((r) => r.id === intent.id)?.plan
          if (!plan || plan.lineage !== lineage) throw new Error('recovery plan missing')
          intent.plan = plan
        }
        if (client !== this.client || this.destroyed || !client.isSynced) return
        client.completeRecovery(intent.id, plan.update)
      }
    } finally { if (this.replaying === client) this.replaying = null }
  }

  private async finishRecovery() {
    if (this.completing === this.client || !this.recovering || !this.client.isConfirmed || !this.client.isSynced) return
    const client = this.client
    this.completing = client
    try {
      await this.persistence?.waitStored()
      if (this.destroyed || client !== this.client || !client.isConfirmed || !client.isSynced) return
      const ids = new Set(this.recovery.map((r) => r.id))
      const lineage = this.lineage
      await this.meta.updateRecoveries(this.id, (list) => list.map((r) => {
        if (!ids.has(r.id)) return r
        if (r.plan?.lineage !== lineage) throw new Error('recovery lineage changed')
        return { ...r, done: true }
      }))
      if (this.destroyed || client !== this.client || !client.isConfirmed || !client.isSynced) return
      this.persistence?.markClean()
      this.stored.base = this.text.toString()
      await this.meta.put(this.id, this.stored)
      if (this.destroyed || client !== this.client || !client.isConfirmed || !client.isSynced) return
      this.recovery = []
      this.recovering = false
      this.notifyReadOnly()
    } finally { if (this.completing === client) this.completing = null }
  }
}
