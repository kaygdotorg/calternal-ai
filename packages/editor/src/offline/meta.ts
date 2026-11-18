/**
 * What this device remembers about a note between visits, beside the
 * document itself:
 *
 * - `lineage`: which server copy of the note the local document grew from.
 * - `base`: the note's text the last time the server confirmed everything.
 *   If the server's copy is ever replaced, only the changes made since this
 *   base are replayed, never the whole text (that would duplicate it).
 * - `pendingSince`: when the oldest change the server hasn't confirmed was
 *   made, so "Not saved yet" survives a reload and stays honest.
 * - `heldSince`: when the oldest collaborator's change this device holds,
 *   unconfirmed by the server, arrived. Not shown, but signing out warns:
 *   this device may hold its last copy. Optional, so older records read.
 */

export interface NoteMeta {
  lineage: string | null
  base: string | null
  pendingSince: number | null
  heldSince?: number | null
  /** A refusal survives reload until a fresh complete submission is receipted. */
  refused?: 'too_large' | 'error' | null
  /** A durable replay obligation, kept separately from stale sibling metadata. */
  recoverySince?: number | null
}

export interface RecoveryIntent {
  id: string
  at: number
  lineage: string | null
  base: string | null
  local: string
  plan?: { lineage: string; update: Uint8Array; clean: boolean }
  done?: boolean
}

// Completed plans remain as tombstones: a suspended tab must reuse the same
// Yjs writer/update, never generate a second insertion. Fail closed at capacity.
const MAX_RECOVERIES = 32
const MAX_RECOVERY_BYTES = 512 * 1024 * 1024
const recoveryKey = (id: string) => `${id}\u0000recovery`
const recoverySince = (list: RecoveryIntent[]) => list.reduce<number | null>((at, r) => !r.done && (at == null || r.at < at) ? r.at : at, null)
function bounded(list: RecoveryIntent[]): void {
  const bytes = list.reduce((n, r) => n + 2 * ((r.base?.length ?? 0) + r.local.length) + (r.plan?.update.byteLength ?? 0), 0)
  if (list.length > MAX_RECOVERIES || bytes > MAX_RECOVERY_BYTES) throw new Error('recovery storage capacity exceeded')
}

export const EMPTY_META: NoteMeta = { lineage: null, base: null, pendingSince: null }

export interface MetaStore {
  get(id: string): Promise<NoteMeta>
  put(id: string, meta: NoteMeta): Promise<void>
  /** Only a covering receipt can clear this marker; ordinary stale puts cannot. */
  clearRefusal(id: string): Promise<void>
  /** Clears `heldSince` only if it still is `since` (a closed note's receipt arrived). */
  clearHeld(id: string, since: number): Promise<void>
  remove(id: string): Promise<void>
  recoveries(id: string): Promise<RecoveryIntent[]>
  /** Atomic compare-and-update, separate from ordinary last-writer metadata. */
  updateRecoveries(id: string, change: (list: RecoveryIntent[]) => RecoveryIntent[]): Promise<RecoveryIntent[]>
}

const DB = 'calternal-notes-meta'
const STORE = 'notes'

/**
 * The metadata database, opened once per page and shared by every note:
 * opening it again for each note cost a database open per note switch.
 */
let shared: Promise<IDBDatabase> | null = null
function openShared(): Promise<IDBDatabase> {
  shared ??= new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(DB, 1)
    req.onupgradeneeded = () => req.result.createObjectStore(STORE)
    req.onsuccess = () => {
      // Another tab deleting or upgrading it: let go, and open afresh next time.
      req.result.onversionchange = () => {
        req.result.close()
        shared = null
      }
      resolve(req.result)
    }
    req.onerror = () => {
      shared = null
      reject(req.error)
    }
  })
  return shared
}

/** Where a note's `base` is kept: beside the rest, under its own key (see below). */
const baseKey = (id: string) => `${id}\u0000base`

/**
 * Note metadata in IndexedDB.
 *
 * `base` is the note's whole text, and the small fields change on every keystroke (pending, then
 * confirmed). Kept in one record, each of those writes copied the whole text again: for a long
 * note, most of the work a keystroke did (#327). So `base` has a key of its own and is written
 * only when it changed. Records from before still read: a `base` inside the record is used until
 * one is written under its own key.
 */
export function idbMetaStore(): MetaStore {
  const db = openShared
  /** The `base` last written for each note, so an unchanged one isn't written again. */
  const written = new Map<string, string | null>()
  const run = async (mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest | void) => {
    const d = await db()
    return new Promise<unknown>((resolve, reject) => {
      const tx = d.transaction(STORE, mode)
      const req = f(tx.objectStore(STORE))
      tx.oncomplete = () => resolve(req ? req.result : undefined)
      tx.onerror = () => reject(tx.error)
    })
  }
  return {
    async get(id) {
      let own: unknown
      const v = (await run('readonly', (s) => {
        s.get(baseKey(id)).onsuccess = (e) => (own = (e.target as IDBRequest).result)
        return s.get(id)
      })) as Partial<NoteMeta> | undefined
      const meta = { ...EMPTY_META, ...(v ?? {}) }
      // Only a base under its own key counts as written: one still inside an older record is
      // written out on the next put, before the record loses it.
      if (own !== undefined) {
        meta.base = own as string | null
        written.set(id, meta.base)
      }
      return meta
    },
    async put(id, meta) {
      const { base, ...small } = meta
      const changed = !written.has(id) || written.get(id) !== base
      await run('readwrite', (s) => {
        const current = s.get(id)
        current.onsuccess = () => {
          // A stale sibling's metadata write cannot erase a refusal.
          const refused = current.result?.refused
          if (refused === 'too_large' || (refused === 'error' && small.refused !== 'too_large')) small.refused = refused
          if (current.result?.recoverySince !== undefined) small.recoverySince = current.result.recoverySince
          else delete small.recoverySince
          s.put(small, id)
          if (changed) s.put(base, baseKey(id))
        }
      })
      written.set(id, base)
    },
    async remove(id) {
      written.delete(id)
      await run('readwrite', (s) => {
        s.delete(id)
        s.delete(baseKey(id))
        s.delete(recoveryKey(id))
      })
    },
    async clearRefusal(id) {
      await run('readwrite', (s) => {
        const current = s.get(id)
        current.onsuccess = () => {
          if (current.result) s.put({ ...current.result, refused: null }, id)
        }
      })
    },
    async clearHeld(id, since) {
      await run('readwrite', (s) => {
        const current = s.get(id)
        current.onsuccess = () => {
          if (current.result?.heldSince === since) s.put({ ...current.result, heldSince: null }, id)
        }
      })
    },
    async recoveries(id) {
      return (await run('readonly', (s) => s.get(recoveryKey(id))) as RecoveryIntent[] | undefined) ?? []
    },
    async updateRecoveries(id, change) {
      const d = await db()
      return new Promise<RecoveryIntent[]>((resolve, reject) => {
        const tx = d.transaction(STORE, 'readwrite')
        const s = tx.objectStore(STORE)
        let result: RecoveryIntent[]
        let failure: unknown
        const request = s.get(recoveryKey(id))
        request.onsuccess = () => {
          try {
            result = change(request.result ?? [])
            bounded(result)
            s.put(result, recoveryKey(id))
            const current = s.get(id)
            current.onsuccess = () => s.put({ ...(current.result ?? {}), recoverySince: recoverySince(result) }, id)
          } catch (e) { failure = e; tx.abort() }
        }
        tx.oncomplete = () => resolve(result!)
        tx.onerror = tx.onabort = () => reject(failure ?? tx.error)
      })
    },
  }
}

/** Metadata kept in memory only, for tests and private windows. */
export function memoryMetaStore(): MetaStore {
  const m = new Map<string, NoteMeta>()
  const intents = new Map<string, RecoveryIntent[]>()
  return {
    async get(id) {
      return { ...EMPTY_META, ...(m.get(id) ?? {}) }
    },
    async put(id, meta) {
      const refused = m.get(id)?.refused
      m.set(id, { ...meta, ...(intents.has(id) ? { recoverySince: recoverySince(intents.get(id)!) } : {}), ...(refused === 'too_large' || (refused === 'error' && meta.refused !== 'too_large') ? { refused } : {}) })
    },
    async remove(id) {
      m.delete(id)
      intents.delete(id)
    },
    async clearRefusal(id) {
      const current = m.get(id)
      if (current) m.set(id, { ...current, refused: null })
    },
    async clearHeld(id, since) {
      const current = m.get(id)
      if (current?.heldSince === since) m.set(id, { ...current, heldSince: null })
    },
    async recoveries(id) { return structuredClone(intents.get(id) ?? []) },
    async updateRecoveries(id, change) {
      const list = change(structuredClone(intents.get(id) ?? []))
      bounded(list)
      intents.set(id, structuredClone(list))
      m.set(id, { ...EMPTY_META, ...m.get(id), recoverySince: recoverySince(list) })
      return structuredClone(list)
    },
  }
}

/**
 * Asks the browser to keep our storage even when the device runs low.
 * Without this, Safari and Chrome may wipe unsynced edits. Returns whether
 * storage is persistent; failure is never fatal.
 */
export async function requestPersistentStorage(): Promise<boolean> {
  try {
    if (!navigator.storage?.persist) return false
    if (await navigator.storage.persisted()) return true
    return await navigator.storage.persist()
  } catch {
    return false
  }
}
