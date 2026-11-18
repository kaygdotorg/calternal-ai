/**
 * The room's gap rules, as `crates/notes` implements them (room/sync.rs,
 * room/acks.rs, ydoc.rs), shared by the test room and the model's server.
 *
 * - An update or sync step 2 whose changes skip ahead of what the room has
 *   from one of its writers (a gap) is not applied. The room asks the client
 *   to resend (it sends sync step 1) and holds the client's save checks.
 * - An applied message that leaves something waiting (a change that builds
 *   on one the room lacks) asks for a resend too.
 * - Only a sync step 2 that fills every gap releases the held check (the
 *   newest one: answers are cumulative). An ordinary update never does.
 */
import * as Y from 'yjs'

/** `update` carries on from what `doc` has from each of its writers (`ydoc::follows_on`). */
export function followsOn(doc: Y.Doc, update: Uint8Array): boolean {
  const { from } = Y.parseUpdateMeta(update)
  for (const [client, clock] of from) if (clock > Y.getState(doc.store, client)) return false
  return true
}

/** Something applied is still waiting for what it builds on (`ydoc::has_missing`). */
export function hasMissing(doc: Y.Doc): boolean {
  return doc.store.pendingStructs != null || doc.store.pendingDs != null
}
