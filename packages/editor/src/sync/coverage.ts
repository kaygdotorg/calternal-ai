/**
 * What the server is known to hold, as Yjs ids and deletions only.
 *
 * Receipts (`receipts.ts`) use this to tell whether a waiting sibling edit is
 * already on the server. It works like a Yjs "shadow" document fed the same
 * updates, but never stores note text: adding a whole-note proof costs a few
 * ranges, not a 64 MiB apply, and memory stays small.
 *
 * Like Yjs, an update only counts once everything before it from the same
 * writer, and every item it builds on, is known. Until then it waits.
 */
import * as Y from 'yjs'

type Range = { client: number; clock: number; length: number }
/** Content-free evidence of the items and deletions in one submission. */
export type CoverageProof = { ranges: Range[]; ds: Y.Snapshot['ds']; dependencies?: Map<number, number> }

/** Waiting proofs kept at most. Dropping one only costs a repeat upload. */
const MAX_PENDING = 512

function depend(dependencies: Map<number, number>, id: Y.ID | null): void {
  if (id) dependencies.set(id.client, Math.max(dependencies.get(id.client) ?? 0, id.clock + 1))
}
/** The items `struct` is placed against: left and right neighbours, parent. */
function references(dependencies: Map<number, number>, struct: Y.Item | Y.GC): void {
  if (!(struct instanceof Y.Item)) return
  depend(dependencies, struct.origin)
  depend(dependencies, struct.rightOrigin)
  const parent = struct.parent
  depend(dependencies, parent instanceof Y.ID ? parent : parent instanceof Y.AbstractType ? parent._item?.id ?? null : null)
}

/** Proof for a local transaction, read from the document without encoding it. */
export function transactionProof(transaction: Y.Transaction): CoverageProof {
  const ranges: Range[] = []
  const dependencies = new Map<number, number>()
  for (const [client, end] of transaction.afterState) {
    const clock = transaction.beforeState.get(client) ?? 0
    if (end <= clock) continue
    ranges.push({ client, clock, length: end - clock })
    const structs = transaction.doc.store.clients.get(client)!
    for (let i = Y.findIndexSS(structs, clock); i < structs.length; i++) references(dependencies, structs[i]!)
  }
  return { ranges, dependencies, ds: transaction.deleteSet }
}

/** Proof for an encoded update. Throws on a malformed update, like `Y.decodeUpdate`. */
export function updateProof(update: Uint8Array): CoverageProof {
  const { structs, ds } = Y.decodeUpdate(update)
  const dependencies = new Map<number, number>()
  const ranges: Range[] = []
  for (const struct of structs) if (!(struct instanceof Y.Skip)) {
    ranges.push({ client: struct.id.client, clock: struct.id.clock, length: struct.length })
    references(dependencies, struct)
  }
  return { ranges, dependencies, ds }
}

export class Coverage {
  private sv = new Map<number, number>()
  private ds = Y.createDeleteSet()
  private pending: CoverageProof[] = []

  /** A Yjs snapshot of what is known, for `Y.snapshotContainsUpdate`. */
  get snapshot(): Y.Snapshot {
    // A deletion for a missing item is pending in Yjs, too. It must not
    // prove coverage before that item is known.
    const ds = Y.createDeleteSet()
    for (const [client, ranges] of this.ds.clients) {
      const end = this.sv.get(client) ?? 0
      const known = ranges.filter((r) => r.clock < end).map((r) => ({ clock: r.clock, len: Math.min(r.len, end - r.clock) }))
      if (known.length) ds.clients.set(client, known)
    }
    return Y.createSnapshot(ds, this.sv)
  }

  /** Adds a proof. A Yjs snapshot of a whole document needs nothing before it. */
  add(proof: CoverageProof | Y.Snapshot): void {
    this.ds = Y.mergeDeleteSets([this.ds, proof.ds])
    this.pending.push('ranges' in proof ? proof : { ranges: [...proof.sv].map(([client, length]) => ({ client, clock: 0, length })), ds: proof.ds })
    // Each counted proof can unblock others; repeat until nothing changes.
    let progress = true
    while (progress) {
      progress = false
      this.pending = this.pending.filter((entry) => {
        const ends = new Map(this.sv)
        for (const { client, clock, length } of entry.ranges) {
          if (clock > (ends.get(client) ?? 0)) return true
          ends.set(client, Math.max(ends.get(client) ?? 0, clock + length))
        }
        for (const [client, end] of entry.dependencies ?? []) if ((ends.get(client) ?? 0) < end) return true
        for (const [client, end] of ends) this.sv.set(client, end)
        progress = true
        return false
      })
    }
    if (this.pending.length > MAX_PENDING) this.pending.splice(0, this.pending.length - MAX_PENDING)
  }

  /** Adds an update the server sent. Returns its proof. */
  update(update: Uint8Array): CoverageProof {
    const proof = updateProof(update)
    this.add(proof)
    return proof
  }
}
