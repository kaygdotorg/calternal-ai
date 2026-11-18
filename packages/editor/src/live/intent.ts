/**
 * What this person wrote themselves, kept apart from what everyone else wrote, for the one case that needs it: access to a shared note was taken away while they had edits the room had not confirmed, and those edits
 * (and only those) are kept for them as a note of their own (`sharing.rescue`). The merged text cannot say who wrote what, and diffing it against an old copy would also pick up other people's changes, which are not this
 * person's to take away. Yjs does know: every inserted run carries the client that made it. This document's own client id is this person's editor (typing, paste, undo and redo all insert under it; a peer, the room and
 * another tab are other clients), and a clock says which of those insertions the room has not yet confirmed.
 *
 * `blocks()` walks the text as it is now and returns the Markdown blocks made of exactly the characters this client inserted since the last confirmation and that are still there. Another person's words typed inside the
 * run are left out; two pieces of one block with only such words between them are joined with a space, and pieces separated by a line break stay separate blocks. A deletion adds nothing. `clear()` is called when the room
 * confirms this person's edits (they are durable, so nothing is owed). Cost: one pass over the items of the text, only at the moment access ends; nothing runs while typing. It reads Yjs's item list (`_start`, `right`), which
 * is stable within the pinned 13.x; the unit tests would fail first if an upgrade changed it. Not covered: edits this person made in an earlier visit and that came back from this device's copy (another client id): the
 * copy keeps them and the session's own recovery handles them.
 */
import * as Y from 'yjs'

/** The Markdown blocks of `text`: trimmed, non-empty, split at blank lines. */
const blocksOf = (text: string): string[] => text.split(/\n[ \t]*\n/).map((b) => b.trim()).filter(Boolean)

export class LocalIntent {
  /** This client's clock when the room last confirmed its edits: only insertions at or after it are owed. */
  private confirmed = 0

  constructor(private readonly text: Y.Text) {
    this.clear()
  }

  /** The room confirmed this person's edits: nothing is owed up to now. */
  clear(): void {
    const doc = this.text.doc
    this.confirmed = doc ? (Y.decodeStateVector(Y.encodeStateVector(doc)).get(doc.clientID) ?? 0) : 0
  }

  /** What is owed: this person's own unconfirmed insertions as Markdown blocks (empty when there are none). */
  blocks(): string[] {
    const doc = this.text.doc
    if (!doc) return []
    const me = doc.clientID
    const pieces: string[] = []
    let cur = ''
    let gap = ''
    for (let item = this.text._start as Y.Item | null; item; item = item.right as Y.Item | null) {
      if (item.deleted || !(item.content instanceof Y.ContentString)) continue
      // Yjs merges neighbouring insertions of one client into one item, so an item can start before the confirmation and run past it: only its part from the confirmation on is owed.
      const from = Math.max(0, this.confirmed - item.id.clock)
      const own = item.id.client === me && from < item.length
      if (own) {
        // Others' words between two of ours that do not break the block are dropped and the pieces joined with a space.
        let str = item.content.str.slice(from)
        if (cur && gap && !/\n/.test(gap)) { cur = cur.trimEnd() + ' '; str = str.trimStart() }
        else if (cur && gap) { pieces.push(cur); cur = '' }
        gap = ''
        cur += str
      } else if (cur) gap += item.content.str
    }
    if (cur) pieces.push(cur)
    return blocksOf(pieces.join('\n\n'))
  }
}
