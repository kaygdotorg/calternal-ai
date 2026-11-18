import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { anchorAt, indexOf, lineHash } from '../src/sync/anchor'

describe('anchors', () => {
  it('follow their block when lines are added above it, on this or another device', () => {
    const a = new Y.Doc()
    const text = a.getText('md')
    text.insert(0, '# Note\n\n## Plan\nStep one\n')
    const at = anchorAt(text, text.toString().indexOf('## Plan'))

    // Another device adds lines above; the change arrives here.
    const b = new Y.Doc()
    Y.applyUpdate(b, Y.encodeStateAsUpdate(a))
    b.getText('md').insert(8, 'new line\nanother\n')
    Y.applyUpdate(a, Y.encodeStateAsUpdate(b))

    const now = indexOf(text, at)!
    expect(text.toString().slice(now, now + 7)).toBe('## Plan')
  })

  it('say when their text is gone', () => {
    const doc = new Y.Doc()
    const text = doc.getText('md')
    text.insert(0, 'keep\ngone\n')
    const at = anchorAt(text, 5)
    text.delete(5, 5)
    // The position falls to where the text was: the server's hash check catches the change.
    expect(indexOf(text, at)).toBe(5)
  })

  it('hash a line as lowercase hex SHA-256', async () => {
    expect(await lineHash('## Plan')).toMatch(/^[0-9a-f]{64}$/)
    expect(await lineHash('## Plan')).not.toBe(await lineHash('## Plan '))
    expect(await lineHash('')).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855')
  })
})
