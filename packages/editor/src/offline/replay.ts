/**
 * Replaying offline changes onto a server copy that was replaced.
 *
 * Rarely, the server's copy of a note starts a new lineage (restored from a
 * backup, migrated, re-seeded from the file). A device that was offline
 * still holds the old lineage. Merging the two documents would insert the
 * whole text twice, so instead we take only what this device changed since
 * the server last confirmed it (`base` → `local`) and apply that as a patch
 * to the new server text.
 */

import DiffMatchPatch from 'diff-match-patch'

export interface ReplayResult {
  /** The text to end up with. */
  text: string
  /** False when some local changes couldn't be placed; keep a copy then. */
  clean: boolean
}

export function replay(base: string, local: string, server: string): ReplayResult {
  if (base === local) return { text: server, clean: true }
  const dmp = new DiffMatchPatch()
  const patches = dmp.patch_make(base, local)
  const [text, applied] = dmp.patch_apply(patches, server)
  return { text, clean: applied.every(Boolean) }
}

/** The smallest single edit turning `from` into `to`. */
export function minimalEdit(from: string, to: string): { at: number; remove: number; insert: string } {
  let start = 0
  const max = Math.min(from.length, to.length)
  while (start < max && from.charCodeAt(start) === to.charCodeAt(start)) start++
  let endFrom = from.length
  let endTo = to.length
  while (endFrom > start && endTo > start && from.charCodeAt(endFrom - 1) === to.charCodeAt(endTo - 1)) {
    endFrom--
    endTo--
  }
  // Never cut a surrogate pair (an emoji) in half.
  if (start > 0 && isHigh(from.charCodeAt(start - 1))) start--
  if (endFrom < from.length && isLow(from.charCodeAt(endFrom))) {
    endFrom++
    endTo++
  }
  return { at: start, remove: endFrom - start, insert: to.slice(start, endTo) }
}

const isHigh = (c: number) => c >= 0xd800 && c <= 0xdbff
const isLow = (c: number) => c >= 0xdc00 && c <= 0xdfff
