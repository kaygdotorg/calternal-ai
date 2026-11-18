/**
 * Anchors: a place in a note that stays put while text around it changes (#80). The server finds
 * a block by its anchor in the live note, so a change on another device can't make it act on a
 * different block.
 */
import * as Y from 'yjs'

/** base64 of a Yjs relative position at `index` of `text`. */
export function anchorAt(text: Y.Text, index: number): string {
  const bytes = Y.encodeRelativePosition(Y.createRelativePositionFromTypeIndex(text, index))
  let s = ''
  for (const b of bytes) s += String.fromCharCode(b)
  return btoa(s)
}

/** Where an anchor made by {@link anchorAt} is now, or null if its text is gone. */
export function indexOf(text: Y.Text, anchor: string): number | null {
  const bytes = Uint8Array.from(atob(anchor), (c) => c.charCodeAt(0))
  const rel = Y.decodeRelativePosition(bytes)
  const abs = text.doc ? Y.createAbsolutePositionFromRelativePosition(rel, text.doc) : null
  return abs && abs.type === text ? abs.index : null
}

/** Lowercase hex SHA-256 of `line`: the server checks the block's first line is still this. */
export async function lineHash(line: string): Promise<string> {
  const d = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(line))
  return [...new Uint8Array(d)].map((b) => b.toString(16).padStart(2, '0')).join('')
}
