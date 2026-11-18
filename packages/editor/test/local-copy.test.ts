import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { LocalCopy } from '../src/offline/local-copy'

const tick = (ms = 0) => new Promise((r) => setTimeout(r, ms))
const name = () => `calternal-note-test-${Math.random().toString(36).slice(2)}`

describe('local copy', () => {
  it('reopens with every change', async () => {
    const n = name()
    const a = new Y.Doc()
    const ca = await LocalCopy.open(n, a)
    a.getText('md').insert(0, 'hello')
    a.getText('md').insert(5, ' world')
    await tick(20)
    ca.destroy()
    const b = new Y.Doc()
    await LocalCopy.open(n, b)
    expect(b.getText('md').toString()).toBe('hello world')
  })

  it('rescues changes from the write-ahead list when IndexedDB never got them', async () => {
    const n = name()
    const a = new Y.Doc()
    const ca = await LocalCopy.open(n, a)
    a.getText('md').insert(0, 'kept')
    await tick(20)
    // Simulate the tab closing before IndexedDB finished: the change exists
    // only in the synchronous list.
    const lost = new Y.Doc()
    lost.getText('md').insert(0, 'x')
    Y.applyUpdate(lost, Y.encodeStateAsUpdate(a))
    const extra = Y.encodeStateAsUpdate(lost, Y.encodeStateVector(a))
    let s = ''
    extra.forEach((c) => (s += String.fromCharCode(c)))
    localStorage.setItem(`${n}:wal:gone`, JSON.stringify([{ n: 1, u: btoa(s) }]))
    ca.destroy()

    const b = new Y.Doc()
    await LocalCopy.open(n, b)
    expect(b.getText('md').toString()).toContain('kept')
    expect(b.getText('md').toString()).toContain('x')
    expect(localStorage.getItem(`${n}:wal:gone`)).toBeNull()
  })

  it('clears the write-ahead list once IndexedDB has the change', async () => {
    const n = name()
    const a = new Y.Doc()
    await LocalCopy.open(n, a)
    a.getText('md').insert(0, 'x')
    await tick(30)
    const left = Object.keys(localStorage).filter((k) => k.startsWith(`${n}:wal:`))
    expect(left).toEqual([])
  })

  it('saves unconfirmed changes synchronously when the page goes away', async () => {
    const n = name()
    const a = new Y.Doc()
    await LocalCopy.open(n, a)
    a.getText('md').insert(0, 'last letters')
    // Same tick: IndexedDB hasn't confirmed yet.
    window.dispatchEvent(new Event('pagehide'))
    const keys = Object.keys(localStorage).filter((k) => k.startsWith(`${n}:wal:`))
    expect(keys.length).toBe(1)
    await tick(30)
    // Once IndexedDB confirms, the list is emptied again.
    expect(Object.keys(localStorage).filter((k) => k.startsWith(`${n}:wal:`))).toEqual([])
  })

  it('compacting never drops another tab’s changes', async () => {
    const n = name()
    const a = new Y.Doc()
    const b = new Y.Doc()
    await LocalCopy.open(n, a)
    await LocalCopy.open(n, b)
    b.getText('md').insert(0, 'from b ')
    for (let i = 0; i < 520; i++) a.getText('md').insert(a.getText('md').length, '.')
    await tick(100)
    const c = new Y.Doc()
    await LocalCopy.open(n, c)
    expect(c.getText('md').toString()).toContain('from b')
    expect(c.getText('md').toString().split('.').length - 1).toBe(520)
  })
})
