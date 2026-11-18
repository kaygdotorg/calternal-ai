import { expect, it } from 'vitest'
import { Awareness } from 'y-protocols/awareness'
import * as Y from 'yjs'
import { SyncClient } from '../src/sync/client'
import { ImeGate } from '../src/sync/gate'
import { memoryTransport } from '../test-server/memory'
import { Room } from '../test-server/room'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
async function until(f: () => boolean, ms = 2000) {
  const end = Date.now() + ms
  while (!f()) {
    if (Date.now() > end) throw new Error('timed out')
    await tick(5)
  }
}
function person(room: Room, account: string) {
  const doc = new Y.Doc()
  const awareness = new Awareness(doc)
  awareness.setLocalState({ account_id: account })
  const c = new SyncClient({ doc, transport: memoryTransport(() => room), gate: new ImeGate(), channel: null, awareness, checkDelayMs: 5 })
  c.connect()
  return { doc, awareness, c }
}
const others = (a: Awareness) => [...a.getStates()].filter(([id]) => id !== a.doc.clientID).map(([, s]) => s.account_id)

it('people in a room see each other, a moved cursor, and the other leaving', async () => {
  const room = new Room('hello')
  const a = person(room, 'acc-a')
  const b = person(room, 'acc-b')
  await until(() => others(a.awareness).includes('acc-b') && others(b.awareness).includes('acc-a'))
  // A cursor travels as a relative position and still means the same place after the text changes.
  const text = a.doc.getText('md')
  await until(() => b.doc.getText('md').toString() === 'hello')
  a.awareness.setLocalStateField('cursor', { anchor: Y.relativePositionToJSON(Y.createRelativePositionFromTypeIndex(text, 3)), head: null })
  await until(() => (b.awareness.getStates().get(a.doc.clientID)?.cursor as { anchor?: unknown } | undefined)?.anchor != null)
  a.c.destroy()
  await until(() => !others(b.awareness).includes('acc-a'))
  b.c.destroy()
})

it('someone who joins later is told who is already here', async () => {
  const room = new Room('x')
  const a = person(room, 'acc-a')
  await until(() => a.c.isConfirmed)
  const b = person(room, 'acc-b')
  await until(() => others(b.awareness).includes('acc-a'))
  a.c.destroy(); b.c.destroy()
})
