/**
 * Which lines make up an item's branch (#247): the item's own line and every line under it, down to
 * where the next item at its level (or a shallower one) starts. That is the stretch of guides that
 * lights up in the accent when the item holds the caret, is selected, or is under the pointer.
 *
 * Pure: it works on the depth of each line, as `treeRegion` and `treeShape` already worked it out,
 * so a selection change costs one short walk and no new parsing.
 */

/** Depth of a list item's own line. */
export const CONTINUATION = -1
/** A line outside any list. */
export const OUTSIDE = -2

/**
 * `depths[i]` is the item's depth on an item's line, `CONTINUATION` on a line that belongs to the
 * item above (wrapped text, a blank line), `OUTSIDE` off the list. Given any line `i`, returns the
 * `[from, to)` lines of the branch it belongs to, and the depth of the branch's head item.
 */
export function branchRange(depths: readonly number[], i: number): { from: number; to: number; depth: number } | null {
  if (i < 0 || i >= depths.length) return null
  let head = i
  while (head >= 0 && depths[head] === CONTINUATION) head--
  const depth = head >= 0 ? depths[head]! : OUTSIDE
  if (depth < 0) return null
  let to = head + 1
  while (to < depths.length && depths[to] !== OUTSIDE && (depths[to] === CONTINUATION || depths[to]! > depth)) to++
  return { from: head, to, depth }
}
