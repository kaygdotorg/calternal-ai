import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { NoteSession, pendingNotes } from '../../src/note'
import { memoryMetaStore } from '../../src/offline/meta'
import { SyncClient } from '../../src/sync/client'
import { ImeGate } from '../../src/sync/gate'
import { memoryTransport } from '../../test-server/memory'
import { Room } from '../../test-server/room'
const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 5000) { const end = Date.now() + ms; while (!f()) { if (Date.now() > end) throw new Error('timed out'); await tick(5) } }
describe('review4 held', () => {
  it('R4-HELD-BUSY: normal collaborator typing does not keep the sign-out warning open', async () => {
    const room = new Room('start\n')
    room.checkDelayMs = 20
    const meta = memoryMetaStore()
    const s = await NoteSession.open({ id: `hb-${Math.random()}`, offline: false, meta, transport: () => memoryTransport(() => room) })
    const tdoc = new Y.Doc()
    const typist = new SyncClient({ doc: tdoc, transport: memoryTransport(() => room), gate: new ImeGate() })
    try {
      s.connect(); typist.connect()
      await until(() => s.client.isConfirmed && typist.isConfirmed)
      let stop = false
      const typing = (async () => { while (!stop) { tdoc.getText('md').insert(tdoc.getText('md').length, 'x'); await tick(50) } })()
      await tick(500)
      // What unsentChanges() does: poll every 50 ms for up to 3 s.
      let clearAt: number | null = null
      const t0 = Date.now()
      while (Date.now() - t0 < 3000) { if (!pendingNotes()) { clearAt = Date.now() - t0; break } await tick(50) }
      stop = true; await typing
      expect(clearAt).not.toBeNull()
      console.log('R4-HELD-BUSY', JSON.stringify({ pendingNotes_cleared_within_3s_at_ms: clearAt, state: s.status.get().state }))
    } finally { s.destroy(); typist.destroy() }
  })
})
