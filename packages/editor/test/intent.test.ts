import { expect, it } from 'vitest'
import * as Y from 'yjs'
import { LocalIntent } from '../src/live/intent'

/** Two replicas of one note, `me` (this person) and `peer`, that exchange updates on demand. */
function pair(text: string) {
  const me = new Y.Doc()
  const peer = new Y.Doc()
  me.getText('md').insert(0, text)
  Y.applyUpdate(peer, Y.encodeStateAsUpdate(me))
  const sync = () => { Y.applyUpdate(peer, Y.encodeStateAsUpdate(me)); Y.applyUpdate(me, Y.encodeStateAsUpdate(peer)) }
  const mine = me.getText('md')
  const intent = new LocalIntent(mine)
  return { me, peer, mine, theirs: peer.getText('md'), sync, intent }
}

it('rescue holds only what this person wrote: one local change, then a later remote change, then access ends', () => {
  const { mine, theirs, sync, intent } = pair('# Plan\n\nOne.')
  mine.insert(mine.length, '\n\nMine, not yet confirmed.')
  theirs.insert(theirs.length, '\n\nA peer wrote this later.')
  sync()
  expect(mine.toString()).toContain('A peer wrote this later.')
  expect(intent.blocks()).toEqual(['Mine, not yet confirmed.'])
})

it("a peer's words typed inside this person's run are left out, and the two halves are joined", () => {
  const { mine, theirs, sync, intent } = pair('Start.')
  mine.insert(mine.length, ' hello world')
  sync()
  theirs.insert(theirs.toString().indexOf(' world'), ' PEER')
  sync()
  expect(mine.toString()).toBe('Start. hello PEER world')
  expect(intent.blocks()).toEqual(['hello world'])
})

it('text already in the note when the person began is not owed, and a deletion adds nothing', () => {
  const { mine, intent } = pair('A\n\nB')
  mine.delete(0, 1)
  expect(intent.blocks()).toEqual([])
  mine.insert(mine.length, '\n\ntyped')
  expect(intent.blocks()).toEqual(['typed'])
})

it('what the room confirmed is no longer owed', () => {
  const { mine, intent } = pair('x')
  mine.insert(1, ' first')
  intent.clear()
  mine.insert(mine.length, '\n\nsecond')
  expect(intent.blocks()).toEqual(['second'])
})

it("another person's own paragraphs are never taken, even next to this person's", () => {
  const { mine, theirs, sync, intent } = pair('Top.')
  theirs.insert(theirs.length, '\n\nTheirs alone.')
  sync()
  mine.insert(mine.length, '\n\nMine alone.')
  expect(intent.blocks()).toEqual(['Mine alone.'])
})
