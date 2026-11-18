import { expect, it } from 'vitest'
import * as Y from 'yjs'
import { Coverage, transactionProof } from '../src/sync/coverage'

it('content-free proofs wait for missing structs, cross-client parents, and deletes', () => {
  const a = new Y.Doc(), b = new Y.Doc(), shadow = new Y.Doc()
  const coverage = new Coverage()
  const updates: Uint8Array[] = []
  a.on('update', (u) => updates.push(u))
  b.on('update', (u, origin) => { if (origin !== 'seed') updates.push(u) })
  const nested = new Y.Text()
  a.getMap('root').set('nested', nested)
  Y.applyUpdate(b, Y.encodeStateAsUpdate(a), 'seed')
  ;(b.getMap('root').get('nested') as Y.Text).insert(0, 'child')
  nested.insert(0, 'parent')
  nested.delete(0, 6)
  const reordered = [updates[1]!, updates[3]!, updates[2]!, updates[0]!]
  for (const update of reordered) {
    coverage.update(update); Y.applyUpdate(shadow, update)
    for (const candidate of updates) {
      if (Y.snapshotContainsUpdate(coverage.snapshot, candidate)) expect(Y.snapshotContainsUpdate(Y.snapshot(shadow), candidate)).toBe(true)
    }
  }
  expect(Y.equalSnapshots(coverage.snapshot, Y.snapshot(shadow))).toBe(true)
  a.destroy(); b.destroy(); shadow.destroy()
})

it('transaction proofs preserve byte coverage for unicode, undo, redo and reordered submissions', () => {
  const doc = new Y.Doc(), shadow = new Y.Doc()
  const text = doc.getText('md'), undo = new Y.UndoManager(text)
  const coverage = new Coverage()
  const updates: Array<{ update: Uint8Array; proof: ReturnType<typeof transactionProof> }> = []
  doc.on('update', (update, _origin, _doc, tr) => updates.push({ update, proof: transactionProof(tr) }))
  text.insert(0, '🙂'.repeat(100)); undo.stopCapturing()
  text.insert(100, 'middle'); undo.stopCapturing()
  text.delete(0, 100); undo.stopCapturing(); undo.undo(); undo.redo()
  for (const { update, proof } of updates.reverse()) { coverage.add(proof); Y.applyUpdate(shadow, update) }
  expect(Y.equalSnapshots(coverage.snapshot, Y.snapshot(shadow))).toBe(true)
  undo.destroy(); doc.destroy(); shadow.destroy()
})
