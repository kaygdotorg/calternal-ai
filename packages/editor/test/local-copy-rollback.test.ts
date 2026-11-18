import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { LocalCopy } from '../src/offline/local-copy'
import { LocalCopy as OldLocalCopy } from './fixtures/local-copy-r2'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
const name = () => `calternal-note-rollback-${Math.random().toString(36).slice(2)}`

async function until(f: () => boolean | Promise<boolean>, ms = 2000) {
  const end = Date.now() + ms
  while (!(await f())) {
    if (Date.now() > end) throw new Error('timed out')
    await tick(5)
  }
}

describe('local copy, rolled back (review 3, R3-ROLLBACK)', () => {
  it('an older client opens and compacts a copy marked as holding unconfirmed edits', async () => {
    const n = name()
    // The current client: an edit made here marks the copy.
    const a = new Y.Doc()
    const current = await LocalCopy.open(n, a)
    a.getText('md').insert(0, 'unsaved offline words')
    await until(async () => {
      const probe = await LocalCopy.open(n, new Y.Doc())
      const marked = probe.dirtySince != null
      probe.destroy()
      return marked
    })
    current.destroy()

    // The client from before: opens it without an error and has the text.
    const b = new Y.Doc()
    const old = await OldLocalCopy.open(n, b)
    expect(b.getText('md').toString()).toBe('unsaved offline words')
    // Enough changes to make the old client compact the store (marker included).
    for (let i = 0; i < 520; i++) b.getText('md').insert(b.getText('md').length, '.')
    const expected = b.getText('md').toString()
    await until(async () => {
      const probe = new Y.Doc()
      const copy = await OldLocalCopy.open(n, probe)
      const ok = probe.getText('md').toString() === expected
      copy.destroy()
      return ok
    })
    old.destroy()

    // And the current client still opens what the old one left.
    const c = new Y.Doc()
    const again = await LocalCopy.open(n, c)
    expect(c.getText('md').toString()).toBe(expected)
    again.destroy()
  })
})
