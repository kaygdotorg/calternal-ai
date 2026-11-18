/**
 * A note's local copy while no editor has it open: read it (with its write-ahead list) and delete it.
 *
 * Why its own file: only the end of a share needs these (rescuing your edits, then removing the copy), so
 * they load with the sharing code, after the first screen, instead of with every editor (#876, first paint).
 * The open editor's copy is `LocalCopy`; the stored format is the same.
 */
import * as Y from 'yjs'
import { DIRTY, LocalCopy, STORE, dirtyKeys, fromB64, storage, updateKeys } from './local-copy'

function openExistingDb(name: string): Promise<IDBDatabase | null> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(name, 1)
    let created = false
    req.onupgradeneeded = () => {
      created = true
      req.transaction?.abort()
    }
    req.onsuccess = () => {
      if (created) {
        req.result.close()
        resolve(null)
        return
      }
      req.result.onversionchange = () => req.result.close()
      resolve(req.result)
    }
    req.onerror = () => {
      if (created && req.error?.name === 'AbortError') resolve(null)
      else reject(req.error)
    }
  })
}

function walUpdates(name: string): { updates: Uint8Array[]; dirty: number | null } {
  const ls = storage()
  const updates: Uint8Array[] = []
  let dirty: number | null = null
  if (!ls) return { updates, dirty }
  for (let i = 0; i < ls.length; i++) {
    const key = ls.key(i)
    if (!key?.startsWith(`${name}:wal:`)) continue
    const entries = JSON.parse(ls.getItem(key) ?? '[]') as Array<{ u: string; l?: 1 }>
    for (const entry of entries) {
      updates.push(fromB64(entry.u))
      if (entry.l && dirty == null) dirty = Date.now()
    }
  }
  return { updates, dirty }
}

/** Reads a closed copy and its write-ahead list without changing either; returns when it first went unsaved. */
export async function readClosedCopy(name: string, doc: Y.Doc): Promise<number | null> {
  const db = typeof indexedDB === 'undefined' ? null : await openExistingDb(name)
  try {
    let updates: Uint8Array[] = []
    let dirty: number | null = null
    if (db) {
      const stored = await new Promise<{ updates: Uint8Array[]; dirty: number | null }>((resolve, reject) => {
        const tx = db.transaction(STORE, 'readonly')
        const store = tx.objectStore(STORE)
        const markers = store.getAllKeys(dirtyKeys())
        const values = store.getAll(updateKeys())
        tx.oncomplete = () => {
          const times = (markers.result as string[])
            .map((key) => Number(key.slice(DIRTY.length)))
            .filter(Number.isFinite)
          resolve({ updates: values.result as Uint8Array[], dirty: times.length ? Math.min(...times) : null })
        }
        tx.onerror = tx.onabort = () => reject(tx.error)
      })
      updates = stored.updates
      dirty = stored.dirty
    }
    const wal = walUpdates(name)
    updates = updates.concat(wal.updates)
    if (wal.dirty != null && (dirty == null || wal.dirty < dirty)) dirty = wal.dirty
    if (!db && !wal.updates.length) return null
    Y.transact(doc, () => updates.forEach((update) => Y.applyUpdate(doc, update)), LocalCopy)
    return dirty
  } finally {
    db?.close()
  }
}

/** Deletes one note's editor database, write-ahead entries, and registry entry. */
export async function forgetCopy(name: string): Promise<void> {
  const ls = storage()
  if (ls) {
    const keys: string[] = []
    for (let i = 0; i < ls.length; i++) {
      const key = ls.key(i)
      if (key?.startsWith(`${name}:wal:`)) keys.push(key)
    }
    keys.forEach((key) => ls.removeItem(key))
    try {
      const names = new Set<string>(JSON.parse(ls.getItem('calternal.note-databases') ?? '[]'))
      names.delete(name)
      ls.setItem('calternal.note-databases', JSON.stringify([...names]))
    } catch {
      // The note data is still removed even if the small registry is unavailable.
    }
  }
  if (typeof indexedDB === 'undefined') return
  await new Promise<void>((resolve, reject) => {
    const req = indexedDB.deleteDatabase(name)
    req.onsuccess = () => resolve()
    req.onerror = () => reject(req.error)
    // A peer tab receives the matching forget signal and closes its session. Keep waiting
    // until the database is actually gone; the control must remain unacknowledged meanwhile.
  })
}
