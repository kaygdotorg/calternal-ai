import { beforeEach, describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { LocalCopy } from '../src/offline/local-copy'
import { readClosedCopy } from '../src/offline/closed-copy'
import { idbMetaStore } from '../src/offline/meta'
import { NoteSession, freezeSharedNote } from '../src/note'
import { forgetNote, pendingSharedEdits, retireSharedCopy } from '../src/shared-copy'

const pause = (ms = 20) => new Promise((resolve) => setTimeout(resolve, ms))
const note = () => `share-recovery-${crypto.randomUUID()}`
async function seed(id: string, text: string): Promise<void> {
  const doc = new Y.Doc()
  const copy = await LocalCopy.open(`calternal-note-${id}`, doc)
  doc.getText('md').insert(0, text)
  await copy.waitStored()
  copy.markClean()
  await pause()
  copy.destroy()
  doc.destroy()
}
const transport = () => ({
  connected: false,
  send: () => {},
  connect: () => {},
  disconnect: () => {},
  onOpen: () => () => {},
  onMessage: () => () => {},
  onClose: () => () => {},
}) as never

describe('shared note recovery', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('reads a closed note copy and returns only the change from its confirmed base', async () => {
    const id = note()
    const meta = idbMetaStore()
    const base = '# Shared\nOriginal line.\n'
    await meta.put(id, { lineage: 'lineage-1', base, pendingSince: null })
    await seed(id, base)
    const doc = new Y.Doc()
    const copy = await LocalCopy.open(`calternal-note-${id}`, doc)
    doc.getText('md').insert(doc.getText('md').length, 'My offline edit.\n')
    await pause()
    copy.destroy()
    doc.destroy()

    const recovery = await pendingSharedEdits(id)
    expect(recovery?.base).toBe('# Shared\nOriginal line.\n')
    expect(recovery?.delta).toContain('My offline edit.')
    expect(recovery?.delta).not.toContain('Original line.')
  })

  it('keeps both removed and added text when an offline edit replaces a line', async () => {
    const id = note()
    const meta = idbMetaStore()
    const base = '# Shared\nRemove this line.\nKeep this line.\n'
    await meta.put(id, { lineage: 'lineage-1', base, pendingSince: null })
    await seed(id, base)
    const doc = new Y.Doc()
    const copy = await LocalCopy.open(`calternal-note-${id}`, doc)
    const text = doc.getText('md')
    const start = text.toString().indexOf('Remove this line.')
    text.delete(start, 'Remove this line.\n'.length)
    text.insert(text.length, 'Add this line.\n')
    await pause()
    copy.destroy()
    doc.destroy()

    const recovery = await pendingSharedEdits(id)
    expect(recovery?.delta).toContain('Add this line.')
    expect(recovery?.delta).toContain('Removed from the shared note:')
    expect(recovery?.delta).toContain('> Remove this line.')
  })

  it('preserves repeated lines in an unsent local edit', async () => {
    const id = note()
    const meta = idbMetaStore()
    const base = '# Shared\nExisting line.\n'
    await meta.put(id, { lineage: 'lineage-1', base, pendingSince: null })
    await seed(id, base)
    const doc = new Y.Doc()
    const copy = await LocalCopy.open(`calternal-note-${id}`, doc)
    doc.getText('md').insert(doc.getText('md').length, 'Repeat me.\nRepeat me.\n')
    await pause()
    copy.destroy()
    doc.destroy()

    const recovery = await pendingSharedEdits(id)
    expect(recovery?.delta.match(/Repeat me\./g)).toHaveLength(2)
  })

  it('returns no rescue for a clean copy and forgets its database, WAL, metadata, and open text', async () => {
    const id = note()
    const meta = idbMetaStore()
    await meta.put(id, { lineage: 'lineage-1', base: '# Shared\nOriginal line.\n', pendingSince: null })
    const cleanDoc = new Y.Doc()
    const clean = await LocalCopy.open(`calternal-note-${id}`, cleanDoc)
    cleanDoc.getText('md').insert(0, '# Shared\nOriginal line.\n')
    clean.markClean()
    await pause()
    clean.destroy()
    cleanDoc.destroy()
    expect(await pendingSharedEdits(id)).toBeNull()

    const session = await NoteSession.open({ id, transport, offline: true, meta })
    session.text.insert(session.text.length, 'Unsynced text.')
    expect((await pendingSharedEdits(id))?.delta).toContain('Unsynced text.')
    await forgetNote(id)
    expect(session.readOnly).toBe(true)
    expect(session.text.toString()).toBe('')
    expect(await meta.get(id)).toEqual({ lineage: null, base: null, pendingSince: null })
    expect(await pendingSharedEdits(id)).toBeNull()
    const list = JSON.parse(localStorage.getItem('calternal.note-databases') ?? '[]') as string[]
    expect(list).not.toContain(`calternal-note-${id}`)
  })

  it('forgets a shared copy without deleting an owned note with the same note ID', async () => {
    const id = note()
    const sharedId = `shared:owner-${crypto.randomUUID()}:${id}`
    const ownText = '# My note\nKeep this copy.\n'
    const sharedText = '# Shared note\nForget only this copy.\n'
    await seed(id, ownText)
    await seed(sharedId, sharedText)

    await forgetNote(sharedId)

    const own = new Y.Doc()
    await readClosedCopy(`calternal-note-${id}`, own)
    expect(own.getText('md').toString()).toBe(ownText)
    own.destroy()

    const shared = new Y.Doc()
    expect(await readClosedCopy(`calternal-note-${sharedId}`, shared)).toBeNull()
    expect(shared.getText('md').toString()).toBe('')
    shared.destroy()
  })

  it('recovers WAL-only keystrokes before forgetting the copy', async () => {
    const id = note()
    const meta = idbMetaStore()
    await meta.put(id, { lineage: 'lineage-1', base: '# Shared\n', pendingSince: null })
    const n = `calternal-note-${id}`
    const doc = new Y.Doc()
    const copy = await LocalCopy.open(n, doc)
    doc.getText('md').insert(doc.getText('md').length, 'Last offline letters.')
    window.dispatchEvent(new Event('pagehide'))
    copy.destroy()
    doc.destroy()

    const recovery = await pendingSharedEdits(id)
    expect(recovery?.delta).toContain('Last offline letters.')
    await forgetNote(id)
    expect(await pendingSharedEdits(id)).toBeNull()
  })

  it('freezes an open note before its pending edits are rescued', async () => {
    const id = note()
    const meta = idbMetaStore()
    const base = '# Shared\nOriginal line.\n'
    await meta.put(id, { lineage: 'lineage-1', base, pendingSince: null })
    await seed(id, base)
    const session = await NoteSession.open({ id, transport, offline: true, meta })
    session.text.insert(session.text.length, 'My offline edit.\n')

    freezeSharedNote(id)

    expect(session.readOnly).toBe(true)
    expect((await pendingSharedEdits(id))?.delta).toContain('My offline edit.')
    await forgetNote(id)
    expect(session.text.toString()).toBe('')
  })

  it('retires pending writes after an edit downgrade and keeps the session read-only', async () => {
    const id = note()
    const meta = idbMetaStore()
    const base = '# Shared\nServer text.\n'
    await meta.put(id, { lineage: 'lineage-1', base, pendingSince: null })
    await seed(id, base)
    const session = await NoteSession.open({ id, transport, offline: true, meta })
    session.text.insert(session.text.length, 'Unsynced edit rescued before downgrade.')
    expect((await pendingSharedEdits(id))?.delta).toContain('Unsynced edit')

    freezeSharedNote(id)
    await retireSharedCopy(id)

    expect(session.readOnly).toBe(true)
    expect(session.text.toString()).not.toContain('Unsynced edit')
    expect(await pendingSharedEdits(id)).toBeNull()
    session.destroy()
  })
})
