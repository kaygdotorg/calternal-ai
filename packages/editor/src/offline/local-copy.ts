/**
 * This device's copy of a note, so it opens instantly and works offline.
 *
 * Every change is written to IndexedDB. IndexedDB writes finish a few
 * milliseconds later, though, and closing the tab in that moment would
 * drop the last keystrokes (a test caught this: type offline, close at
 * once, lose "fline"). So when the page is being hidden or closed, any
 * change IndexedDB hasn't confirmed yet is written synchronously to a
 * small write-ahead list in localStorage. Opening the note applies both;
 * Yjs updates are safe to apply twice. Typing itself never touches
 * localStorage, so it costs nothing per keystroke.
 *
 * The copy also remembers whether it holds an edit made on this device
 * that the server hasn't confirmed: a marker written in the same
 * transaction as that edit, so the two can never be split by a crash.
 * Changes that came from the server or another tab never set it, so a
 * note opened offline is "Not saved yet" only when it has a real edit.
 */

import * as Y from 'yjs'

export const STORE = 'updates'
/**
 * The unconfirmed-edit marker, beside the (numbered) updates: the key
 * `dirty@<ms>` holds when the edit was made, and the value is an empty Yjs
 * update. Clients from before the marker read every value as an update, so
 * to them it is a harmless no-op (and their compaction just drops it).
 */
export const DIRTY = 'dirty@'
export const dirtyKeys = () => IDBKeyRange.bound(DIRTY, `${DIRTY}\uffff`)
const EMPTY_UPDATE = new Uint8Array([0, 0])
/** Only the numbered updates: every number sorts before every string key. */
export const updateKeys = () => IDBKeyRange.upperBound(Number.MAX_VALUE)
/** Past this many stored changes, they're merged into one. */
/** How long a burst of typing waits before its one IndexedDB write. */
const QUEUE_MS = 120
const COMPACT_AT = 500

const toB64 = (u: Uint8Array) => {
  let s = ''
  for (let i = 0; i < u.length; i++) s += String.fromCharCode(u[i]!)
  return btoa(s)
}
export const fromB64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0))

export function storage(): Storage | null {
  try {
    return typeof localStorage === 'undefined' ? null : localStorage
  } catch {
    return null
  }
}

function openDb(name: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(name, 1)
    req.onupgradeneeded = () => req.result.createObjectStore(STORE, { autoIncrement: true })
    req.onsuccess = () => {
      req.result.onversionchange = () => req.result.close()
      resolve(req.result)
    }
    req.onerror = () => reject(req.error)
  })
}



export class LocalCopy {
  /** Whether a burst of typing is written in one transaction a moment after it (the default) or each update at once (the tests that count stored updates, such as compaction, turn it off). */
  static coalesce = true
  private db: IDBDatabase | null
  private readonly walKey: string
  /** Changes not yet confirmed by IndexedDB, oldest first (`l`: a local edit). */
  private pending: Array<{ n: number; u: string; l?: 1 }> = []
  /** When this device's oldest unconfirmed edit was made, or null for none. */
  private dirtyAt: number | null = null
  /** Whether a change of this origin is an edit made here. Set by the owner. */
  isLocal: (origin: unknown) => boolean = () => true
  private seq = 0
  private stored = 0
  private destroyed = false
  /** Whether the write-ahead list in localStorage may hold entries. */
  private walWritten = false
  /** Updates not yet handed to IndexedDB: a burst of typing is written in one transaction a moment after it, not one transaction per key (`flushQueue`). They are also in `pending`, so the write-ahead list covers them. */
  private queued: Uint8Array[] = []
  private queueTimer: ReturnType<typeof setTimeout> | 0 = 0

  private constructor(
    readonly name: string,
    private readonly doc: Y.Doc,
    db: IDBDatabase,
  ) {
    this.db = db
    this.walKey = `${name}:wal:${Math.random().toString(36).slice(2, 10)}`
  }

  /** Opens the copy and loads it into `doc` before returning. */
  static async open(name: string, doc: Y.Doc): Promise<LocalCopy> {
    // Safari versions without database enumeration still need to forget closed notes on sign-out.
    const ls = storage()
    try {
      if (ls) {
        const names = new Set<string>(JSON.parse(ls.getItem('calternal.note-databases') ?? '[]'))
        names.add(name)
        ls.setItem('calternal.note-databases', JSON.stringify([...names]))
      }
    } catch {
      // Full or blocked: the metadata still names this copy for sign-out.
    }
    const copy = new LocalCopy(name, doc, await openDb(name))
    await copy.load()
    doc.on('update', copy.onUpdate)
    if (typeof addEventListener === 'function') {
      addEventListener('pagehide', copy.flush)
      document.addEventListener('visibilitychange', copy.onVisibility)
    }
    return copy
  }

  /** When the oldest edit made here that the server hasn't confirmed was made, if any. */
  get dirtySince(): number | null {
    return this.dirtyAt
  }

  /** The server confirmed every edit made here: forget the marker. */
  markClean(): void {
    if (this.dirtyAt == null || !this.db) return
    this.dirtyAt = null
    try {
      this.db.transaction(STORE, 'readwrite').objectStore(STORE).delete(dirtyKeys())
    } catch {
      // Closing: the marker stays, and the next open checks again.
    }
  }

  /** A receipt may retire recovery storage only after all prior local writes commit. */
  async waitStored(): Promise<void> {
    if (!this.db || this.destroyed) throw new Error('local copy closed')
    // A readonly barrier cannot distinguish an earlier aborted write. This
    // rare recovery path commits the full state before retiring its source.
    await this.put([Y.encodeStateAsUpdate(this.doc)], this.dirtyAt)
  }

  private async load() {
    const { updates, dirty } = await this.all()
    this.dirtyAt = dirty
    this.stored = updates.length
    const ls = storage()
    const walKeys: string[] = []
    const wal: Uint8Array[] = []
    if (ls) {
      for (let i = 0; i < ls.length; i++) {
        const k = ls.key(i)
        if (k?.startsWith(`${this.name}:wal:`)) walKeys.push(k)
      }
      for (const k of walKeys) {
        for (const e of JSON.parse(ls.getItem(k) ?? '[]') as Array<{ u: string; l?: 1 }>) {
          wal.push(fromB64(e.u))
          if (e.l && this.dirtyAt == null) this.dirtyAt = Date.now()
        }
      }
    }
    Y.transact(this.doc, () => updates.concat(wal).forEach((u) => Y.applyUpdate(this.doc, u)), this)
    if (wal.length) {
      // Rescued changes go into IndexedDB for good before the list is cleared.
      await this.put([Y.encodeStateAsUpdate(this.doc)], this.dirtyAt)
      walKeys.forEach((k) => ls?.removeItem(k))
    }
  }

  private all(): Promise<{ updates: Uint8Array[]; dirty: number | null }> {
    return new Promise((resolve, reject) => {
      const store = this.db!.transaction(STORE, 'readonly').objectStore(STORE)
      const marker = store.getAllKeys(dirtyKeys())
      const req = store.getAll(updateKeys())
      req.onsuccess = () => {
        const times = (marker.result as string[]).map((k) => Number(k.slice(DIRTY.length))).filter(Number.isFinite)
        resolve({ updates: req.result as Uint8Array[], dirty: times.length ? Math.min(...times) : null })
      }
      req.onerror = () => reject(req.error)
    })
  }

  /** Stores updates, and the unconfirmed-edit marker with them when given. */
  private put(updates: Uint8Array[], dirty: number | null = null): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.db) return resolve()
      const tx = this.db.transaction(STORE, 'readwrite')
      const store = tx.objectStore(STORE)
      updates.forEach((u) => store.add(u))
      if (dirty != null) store.put(EMPTY_UPDATE, `${DIRTY}${dirty}`)
      tx.oncomplete = () => resolve()
      tx.onerror = () => reject(tx.error)
      tx.onabort = () => reject(tx.error)
    })
  }

  /**
   * Merges the stored changes into one, inside a single transaction. It
   * merges what is stored (not this tab's document), so changes another
   * tab wrote a moment ago are never dropped.
   */
  private compact() {
    if (!this.db) return
    const tx = this.db.transaction(STORE, 'readwrite')
    const store = tx.objectStore(STORE)
    // Only the numbered updates; the marker's key sorts after every number.
    const updates = updateKeys()
    const keysReq = store.getAllKeys(updates)
    const valsReq = store.getAll(updates)
    valsReq.onsuccess = () => {
      const keys = keysReq.result as number[]
      const vals = valsReq.result as Uint8Array[]
      if (keys.length < 2) return
      store.delete(IDBKeyRange.bound(keys[0]!, keys[keys.length - 1]!))
      store.add(Y.mergeUpdates(vals))
    }
  }

  private onUpdate = (update: Uint8Array, origin: unknown) => {
    if (origin === this || this.destroyed) return
    const n = ++this.seq
    // The first unconfirmed local edit writes the marker in its own transaction.
    const local = this.isLocal(origin)
    const mark = local && this.dirtyAt == null ? (this.dirtyAt = Date.now()) : null
    this.pending.push(local ? { n, u: toB64(update), l: 1 } : { n, u: toB64(update) })
    this.queued.push(update)
    // The first unconfirmed edit (which carries the marker) goes at once; the rest of a burst of typing is written together a moment later, off the keystroke.
    if (mark != null || !LocalCopy.coalesce) this.flushQueue(mark)
    else if (!this.queueTimer) this.queueTimer = setTimeout(() => this.flushQueue(), QUEUE_MS)
  }

  /** Hands what is queued to IndexedDB in one transaction (merged into one update); what it confirms leaves `pending`. A failure leaves it in the write-ahead list, which the next open rescues. */
  private flushQueue(mark: number | null = null) {
    if (this.queueTimer) clearTimeout(this.queueTimer)
    this.queueTimer = 0
    const batch = this.queued
    if (!batch.length && mark == null) return
    this.queued = []
    const upto = this.seq
    const write = this.put(batch.length > 1 ? [Y.mergeUpdates(batch)] : batch, mark)
    if (++this.stored >= COMPACT_AT) {
      this.stored = 1
      this.compact()
    }
    write.then(
      () => {
        this.pending = this.pending.filter((e) => e.n > upto)
        if (this.walWritten) this.writeWal()
      },
      () => {
        // Keep it in the write-ahead list; the next open will rescue it.
      },
    )
  }

  /** The page is going away: save what IndexedDB hasn't confirmed, now. */
  private flush = () => {
    this.flushQueue()
    if (this.pending.length || this.walWritten) this.writeWal()
  }

  private onVisibility = () => {
    if (document.visibilityState === 'hidden') this.flush()
  }

  private writeWal() {
    const ls = storage()
    if (!ls) return
    try {
      if (this.pending.length) ls.setItem(this.walKey, JSON.stringify(this.pending))
      else ls.removeItem(this.walKey)
      this.walWritten = this.pending.length > 0
    } catch {
      // Storage full or blocked: IndexedDB still has the change shortly.
    }
  }

  /** Forgets this device's copy entirely (used when the server's copy is replaced). */
  async clear(): Promise<void> {
    this.destroy()
    const ls = storage()
    if (ls) {
      const keys: string[] = []
      for (let i = 0; i < ls.length; i++) {
        const k = ls.key(i)
        if (k?.startsWith(`${this.name}:wal:`)) keys.push(k)
      }
      keys.forEach((k) => ls.removeItem(k))
    }
    await new Promise<void>((resolve, reject) => {
      const req = indexedDB.deleteDatabase(this.name)
      req.onsuccess = () => resolve()
      req.onerror = () => reject(req.error)
      req.onblocked = () => reject(new Error('local copy is open in another tab'))
    })
  }

  destroy(): void {
    if (this.destroyed) return
    // Writes IndexedDB hasn't confirmed yet survive in the write-ahead list.
    this.flush()
    this.destroyed = true
    this.doc.off('update', this.onUpdate)
    if (typeof removeEventListener === 'function') {
      removeEventListener('pagehide', this.flush)
      document.removeEventListener('visibilitychange', this.onVisibility)
    }
    this.db?.close()
    this.db = null
  }
}
