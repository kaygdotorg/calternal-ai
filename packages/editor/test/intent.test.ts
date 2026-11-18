import { expect, it } from 'vitest'
import * as Y from 'yjs'
import { LocalIntent } from '../src/live/intent'

const mine = (o: unknown) => o === 'me'
function room(text: string) {
  const doc = new Y.Doc()
  const t = doc.getText('md')
  t.insert(0, text)
  return { doc, t, intent: new LocalIntent(t, mine) }
}

it('rescue holds only what this person wrote: one local change, then a later remote change, then access ends', () => {
  const { doc, t, intent } = room('# Plan\n\nOne.')
  doc.transact(() => t.insert(t.length, '\n\nMine, not yet confirmed.'), 'me')
  doc.transact(() => t.insert(t.length, '\n\nA peer wrote this later.'), 'peer')
  expect(t.toString()).toContain('A peer wrote this later.')
  expect(intent.blocks()).toEqual(['Mine, not yet confirmed.'])
})

it('a peer\'s words typed inside this person\'s run are left out of it', () => {
  const { doc, t, intent } = room('Start.')
  doc.transact(() => t.insert(t.length, ' hello world'), 'me')
  const at = t.toString().indexOf(' world')
  doc.transact(() => t.insert(at, ' PEER'), 'peer')
  expect(t.toString()).toBe('Start. hello PEER world')
  const got = intent.blocks().join(' ')
  expect(got).not.toContain('PEER')
  expect(got).toContain('hello')
})

it('keystrokes in a row are one run; a delete adds nothing; confirmation clears it', () => {
  const { doc, t, intent } = room('A\n\nB')
  for (const ch of 'typed') doc.transact(() => t.insert(t.length, ch), 'me')
  doc.transact(() => t.delete(0, 1), 'me')
  expect(intent.blocks()).toEqual(['typed'])
  intent.clear()
  expect(intent.blocks()).toEqual([])
})

it('changes that are not this person\'s (the room, another tab, replay) are never noted', () => {
  const { doc, t, intent } = room('x')
  doc.transact(() => t.insert(1, ' from the room'), 'room')
  doc.transact(() => t.insert(t.length, ' from a tab'), 'tab')
  expect(intent.blocks()).toEqual([])
})
