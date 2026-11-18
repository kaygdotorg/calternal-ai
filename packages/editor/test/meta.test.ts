import { describe, expect, it } from 'vitest'
import { idbMetaStore } from '../src/offline/meta'

/** Raw access to the metadata database, to plant an older record and count writes. */
function raw(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open('calternal-notes-meta', 1)
    req.onupgradeneeded = () => req.result.createObjectStore('notes')
    req.onsuccess = () => resolve(req.result)
    req.onerror = () => reject(req.error)
  })
}
async function rawGet(key: string): Promise<unknown> {
  const d = await raw()
  return new Promise((resolve) => {
    const r = d.transaction('notes').objectStore('notes').get(key)
    r.onsuccess = () => resolve(r.result)
  })
}
async function rawPut(key: string, value: unknown): Promise<void> {
  const d = await raw()
  await new Promise<void>((resolve) => {
    const tx = d.transaction('notes', 'readwrite')
    tx.objectStore('notes').put(value, key)
    tx.oncomplete = () => resolve()
  })
}

describe('idbMetaStore', () => {
  it('keeps every field across a fresh store', async () => {
    const a = idbMetaStore()
    await a.get('n1')
    await a.put('n1', { lineage: 'L1', base: 'hello', pendingSince: 5 })
    expect(await idbMetaStore().get('n1')).toEqual({ lineage: 'L1', base: 'hello', pendingSince: 5 })
    await a.put('n1', { lineage: 'L1', base: 'hello', pendingSince: null })
    expect(await idbMetaStore().get('n1')).toEqual({ lineage: 'L1', base: 'hello', pendingSince: null })
  })

  it('writes the whole text only when it changed', async () => {
    const a = idbMetaStore()
    await a.get('n2')
    await a.put('n2', { lineage: 'L', base: 'long text', pendingSince: null })
    // Someone else's copy of the key: an unchanged base must not be written over it again.
    await rawPut('n2\u0000base', 'marker')
    await a.put('n2', { lineage: 'L', base: 'long text', pendingSince: 7 })
    expect(await rawGet('n2\u0000base')).toBe('marker')
    await a.put('n2', { lineage: 'L', base: 'newer text', pendingSince: null })
    expect(await rawGet('n2\u0000base')).toBe('newer text')
    expect(await rawGet('n2')).toEqual({ lineage: 'L', pendingSince: null })
  })

  it('reads a record from before, and never loses its base', async () => {
    await rawPut('n3', { lineage: 'old', base: 'from before', pendingSince: null })
    const a = idbMetaStore()
    expect(await a.get('n3')).toEqual({ lineage: 'old', base: 'from before', pendingSince: null })
    await a.put('n3', { lineage: 'old', base: 'from before', pendingSince: 3 })
    expect(await idbMetaStore().get('n3')).toEqual({ lineage: 'old', base: 'from before', pendingSince: 3 })
  })

  it('forgets both parts', async () => {
    const a = idbMetaStore()
    await a.get('n4')
    await a.put('n4', { lineage: 'L', base: 'x', pendingSince: null })
    await a.remove('n4')
    expect(await rawGet('n4')).toBeUndefined()
    expect(await rawGet('n4\u0000base')).toBeUndefined()
    expect(await idbMetaStore().get('n4')).toEqual({ lineage: null, base: null, pendingSince: null })
  })
})

it('a stale sibling metadata write cannot erase a permanent refusal', async () => {
  const store = idbMetaStore()
  const id = `refused-${Math.random()}`
  const stale = { lineage: 'L', base: 'safe', pendingSince: 1 }
  await store.put(id, { ...stale, refused: 'too_large' })
  await store.put(id, { ...stale, pendingSince: null })
  expect((await store.get(id)).refused).toBe('too_large')
  await store.clearRefusal(id)
  expect((await store.get(id)).refused).toBeNull()
  await store.put(id, { ...stale, pendingSince: null })
  expect((await store.get(id)).refused).toBeUndefined()
  await store.remove(id)
})
