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
 */

import * as Y from 'yjs'

const STORE = 'updates'
/** Past this many stored changes, they're merged into one. */
const COMPACT_AT = 500

const toB64 = (u: Uint8Array) => {
  let s = ''
  for (let i = 0; i < u.length; i++) s += String.fromCharCode(u[i]!)
  return btoa(s)
}
const fromB64 = (s: string) => Uint8Array.from(atob(s), (c) => c.charCodeAt(0))

function storage(): Storage | null {
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
  private db: IDBDatabase | null
  private readonly walKey: string
  /** Changes not yet confirmed by IndexedDB, oldest first. */
  private pending: Array<{ n: number; u: string }> = []
  private seq = 0
  private stored = 0
  private destroyed = false
  /** Whether the write-ahead list in localStorage may hold entries. */
  private walWritten = false

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

  private async load() {
    const updates = await this.all()
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
        for (const e of JSON.parse(ls.getItem(k) ?? '[]') as Array<{ u: string }>) wal.push(fromB64(e.u))
      }
    }
    Y.transact(this.doc, () => updates.concat(wal).forEach((u) => Y.applyUpdate(this.doc, u)), this)
    if (wal.length) {
      // Rescued changes go into IndexedDB for good before the list is cleared.
      await this.put([Y.encodeStateAsUpdate(this.doc)])
      walKeys.forEach((k) => ls?.removeItem(k))
    }
  }

  private all(): Promise<Uint8Array[]> {
    return new Promise((resolve, reject) => {
      const req = this.db!.transaction(STORE, 'readonly').objectStore(STORE).getAll()
      req.onsuccess = () => resolve(req.result as Uint8Array[])
      req.onerror = () => reject(req.error)
    })
  }

  private put(updates: Uint8Array[]): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.db) return resolve()
      const tx = this.db.transaction(STORE, 'readwrite')
      const store = tx.objectStore(STORE)
      updates.forEach((u) => store.add(u))
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
    const keysReq = store.getAllKeys()
    const valsReq = store.getAll()
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
    this.pending.push({ n, u: toB64(update) })
    const write = this.put([update])
    if (++this.stored >= COMPACT_AT) {
      this.stored = 1
      this.compact()
    }
    write.then(
      () => {
        this.pending = this.pending.filter((e) => e.n > n)
        if (this.walWritten) this.writeWal()
      },
      () => {
        // Keep it in the write-ahead list; the next open will rescue it.
      },
    )
  }

  /** The page is going away: save what IndexedDB hasn't confirmed, now. */
  private flush = () => {
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
    await new Promise<void>((resolve) => {
      const req = indexedDB.deleteDatabase(this.name)
      req.onsuccess = req.onerror = req.onblocked = () => resolve()
    })
  }

  destroy(): void {
    if (this.destroyed) return
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
