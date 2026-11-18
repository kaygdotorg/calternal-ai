import { describe, expect, it } from 'vitest'
import * as Y from 'yjs'
import { Receipts } from '../src/sync/receipts'
import { updateProof } from '../src/sync/coverage'

describe('receipt authority', () => {
  it('never lets refusal or an unknown receipt grant Saved', () => {
    const r = new Receipts()
    const n = r.observe('tab1', 1, true)
    r.submitted(n)
    const id = r.check()
    expect(r.receipt(id + 1)).toBe(false)
    r.refuse('too_large')
    r.receipt(id)
    expect(r.confirmed).toBe(false)
    expect(r.status(true).tooLarge).toBe(true)
    r.beginConnection(); r.submittedThrough(r.position); r.receipt(r.check())
    expect(r.confirmed).toBe(false)
    r.destroy()
  })

  it('an edit typed after the sync snapshot needs its own covering submission', () => {
    const r = new Receipts()
    const before = r.observe('tab1', 1, true)
    const through = r.position
    r.observe('tab2', 1, true)
    r.submittedThrough(through)
    r.receipt(r.check())
    expect(r.confirmed).toBe(false)
    r.submitted(before) // Retired entries cannot confirm later ones.
    expect(r.confirmed).toBe(false)
    r.destroy()
  })

  it('a queued IME packet is excluded from a reconnect sync until applied', () => {
    const r = new Receipts()
    const id = r.observe('server', 1, false, Date.now(), undefined, false)
    r.submitted(id)
    r.beginConnection()
    r.submittedThrough(r.position)
    r.receipt(r.check())
    expect(r.confirmed).toBe(false)
    r.applied(id)
    r.submittedThrough(r.position)
    r.receipt(r.check())
    expect(r.confirmed).toBe(true)
    r.destroy()
  })

  it('a sender receipt supplies prerequisites for later sibling coverage', () => {
    const doc = new Y.Doc()
    doc.getText('md').insert(0, 'own words')
    const own = Y.encodeStateAsUpdate(doc)
    const r = new Receipts()
    const id = r.observe('self', 1, true, Date.now(), own)
    r.proof(id, updateProof(own))
    r.submitted(id)
    r.receipt(r.check())
    const peer = new Y.Doc()
    Y.applyUpdate(peer, own)
    let update!: Uint8Array
    peer.on('update', (u) => { update = u })
    peer.getText('md').insert(peer.getText('md').length, ' sibling words')
    r.observeSibling('sibling', 1, update)
    r.serverUpdate(update)
    expect(r.waiting.length).toBe(0)
    expect(r.confirmed).toBe(false)
    r.receipt(r.check())
    expect(r.confirmed).toBe(true)
    r.destroy(); doc.destroy(); peer.destroy()
  })

  it('reopening a room never reuses an earlier membership receipt id', () => {
    const first = new Receipts()
    const old = first.check()
    first.destroy()
    const next = new Receipts()
    const edit = next.observe('next-visit', 1, true)
    next.submitted(edit)
    const current = next.check()
    expect(current).toBeGreaterThan(old)
    expect(next.receipt(old)).toBe(false)
    expect(next.confirmed).toBe(false)
    expect(next.receipt(current)).toBe(true)
    expect(next.confirmed).toBe(true)
    next.destroy()
  })

  it('server coverage includes deletes, and still requires a later receipt', () => {
    const doc = new Y.Doc()
    doc.getText('md').insert(0, 'words')
    const initial = Y.encodeStateAsUpdate(doc)
    let deletion!: Uint8Array
    doc.on('update', (u) => { deletion = u })
    doc.getText('md').delete(0, 5)
    const r = new Receipts()
    r.serverUpdate(initial)
    r.observeSibling('tab2', 1, deletion)
    r.receipt(r.check())
    expect(r.confirmed).toBe(false)
    r.serverUpdate(deletion)
    expect(r.confirmed).toBe(false)
    r.receipt(r.check())
    expect(r.confirmed).toBe(true)
    r.destroy(); doc.destroy()
  })

  it('10,000 random interleavings keep Saved and the safe base behind receipts', () => {
    // Independent oracle stores actual edit identities and the edits present
    // in each server check, rather than deriving coverage from authority state.
    let seed = 65205
    const random = (n: number) => { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; return (seed >>> 0) % n }
    for (let trial = 0; trial < 10000; trial++) {
      type Tab = { r: Receipts; open: boolean; online: boolean; frozen: boolean; edits: Map<number, { token: number; refused: boolean; sent: boolean; integrated: boolean }>; observed: Set<number>; checks: Map<number, number[]>; received: Set<number>; safe: number[] }
      const tabs: Tab[] = Array.from({ length: 1 + random(3) }, () => ({ r: new Receipts(), open: true, online: false, frozen: false, edits: new Map(), observed: new Set(), checks: new Map(), received: new Set(), safe: [] }))
      const server = new Set<number>()
      let token = 0
      const sendCheck = (t: Tab) => {
        const id = t.r.check()
        t.checks.set(id, [...t.edits.values()].filter((e) => e.sent && !e.refused && server.has(e.token)).map((e) => e.token))
      }
      for (let step = 0; step < 100; step++) {
        const t = tabs[random(tabs.length)]!
        switch (random(12)) {
          case 0: case 1: // Typing, including sibling observation before the owner's upload.
            if (!t.open || t.frozen) break
            token++
            for (const dest of tabs.filter((d) => d.open)) {
              const id = dest.r.observe(`tab${tabs.indexOf(t)}`, token, true)
              const sent = dest === t && dest.online
              dest.edits.set(id, { token, refused: false, sent, integrated: true }); dest.observed.add(token)
              if (sent) { server.add(token); dest.r.submitted(id) }
            }
            break
          case 2: // Offline/online; reconnect resubmits held state.
            t.online = !t.online
            t.r.beginConnection(); t.checks.clear()
            for (const [id, e] of t.edits) {
              e.sent = t.online && e.integrated
              if (t.online && e.integrated && !e.refused) { server.add(e.token); t.r.submitted(id) }
            }
            break
          case 3: // A server check covers only already submitted edits.
            if (t.open && t.online) sendCheck(t)
            break
          case 4: // Receipts may be reordered, stale, or forged ids.
            if (t.checks.size && random(4)) {
              const id = [...t.checks.keys()][random(t.checks.size)]!
              const covered = t.checks.get(id)!
              if (t.r.receipt(id)) {
                covered.forEach((n) => t.received.add(n))
                for (const [editId, e] of t.edits) if (covered.includes(e.token) && !e.refused) t.edits.delete(editId)
              }
              for (const key of t.checks.keys()) if (key <= id) t.checks.delete(key)
            } else t.r.receipt(1_000_000 + random(100))
            break
          case 5: { // A newly typed oversize edit, or refusal of an earlier submitted edit.
            const oversize = random(2) === 0
            if (oversize && t.open && !t.frozen) {
              token++
              const id = t.r.observe(`tab${tabs.indexOf(t)}`, token, true)
              t.edits.set(id, { token, refused: false, sent: false, integrated: true }); t.observed.add(token)
            }
            if (t.open && t.edits.size) {
              t.r.refuse(oversize ? 'too_large' : 'error')
              for (const e of t.edits.values()) e.refused = true
            }
            break
          }
          case 6: // Close/open keeps pending edits, drops connection checks.
            t.open = !t.open; t.r.beginConnection(); t.checks.clear()
            for (const e of t.edits.values()) e.sent = false
            break
          case 7: // Sign-out freezes all tabs before any asynchronous work.
            for (const d of tabs) d.frozen = true
            break
          case 8: // Server restart discards all unreceipted server state.
            server.clear(); tabs.forEach((d) => d.received.forEach((n) => server.add(n)))
            for (const d of tabs) { d.r.beginConnection(); d.checks.clear(); for (const e of d.edits.values()) e.sent = false }
            break
          case 9: // Sibling hint/echo: only a server echo grants submission coverage.
            for (const [id, e] of t.edits) if (e.integrated && server.has(e.token) && !e.refused) { t.r.submitted(id); e.sent = true }
            break
          case 10: { // Replacement persists pending intent outside a fresh document.
            const pending = [...t.edits.values()]
            const failure = t.r.refusal
            t.r.destroy()
            t.r = new Receipts(null, null, failure)
            t.edits.clear(); t.checks.clear()
            for (const e of pending) {
              const id = t.r.observe('recovery', e.token, true, Date.now(), undefined, false)
              t.edits.set(id, { ...e, sent: false, integrated: false })
            }
            break
          }
          case 11: // Only replay integration plus upload can submit an outside intent.
            for (const [id, e] of t.edits) if (!e.integrated && !e.refused) {
              t.r.applied(id); e.integrated = true
              if (t.online) { server.add(e.token); t.r.submitted(id); e.sent = true }
              break
            }
            break
        }
        for (const d of tabs) {
          if (d.r.status(d.online).state === 'saved') {
            if (!d.r.confirmed || d.edits.size) throw new Error(`Unreceipted Saved at trial ${trial}, step ${step}`)
            d.safe = [...d.observed]
          }
          // Safe base can only contain identities acknowledged by real receipts.
          for (const n of d.safe) if (!d.received.has(n)) throw new Error(`Unreceipted base at trial ${trial}, step ${step}`)
        }
      }
      tabs.forEach((t) => t.r.destroy())
    }
  }, 180000)
})

describe('persisted refusal', () => {
  it('reloading cannot clear a refusal even after a successful full resend and receipt', () => {
    const before = new Receipts(1000)
    before.refuse('too_large')
    const restored = new Receipts(before.pendingSince, before.heldSince, before.refusal)
    restored.submittedThrough(restored.position)
    restored.receipt(restored.check())
    expect(restored.confirmed).toBe(false)
    expect(restored.status(true).tooLarge).toBe(true)
    before.destroy(); restored.destroy()
  })
})
