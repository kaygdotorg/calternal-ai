/**
 * What this person wrote themselves, kept apart from what everyone else wrote, for the one case that needs it: access to a shared note was taken away while they had edits the room had not confirmed, and those edits
 * (and only those) are kept for them as a note of their own (`sharing.rescue`). The shared text cannot say who wrote what once it is merged, and diffing it against an old copy would also pick up other people's changes, which
 * are not this person's to take away. So every insertion made by this editor (typing, paste, undo and redo; see `mine`) is noted as it happens, as a pair of Yjs relative positions that follow the text as others edit around it.
 *
 * `blocks()` resolves them against the text as it is now and returns, in order, the Markdown blocks made of exactly the characters this person inserted and that are still there: a peer's words inside the run are left out,
 * and a deletion adds nothing. `clear()` is called when the room confirms this person's edits (they are durable, so nothing is owed). Cost: one relative-position pair per run of typing (adjacent typing extends the last run),
 * dropped at every confirmation; no work for a change that is not this person's.
 */
import * as Y from 'yjs'

/** The Markdown blocks of `text`: trimmed, non-empty, split at blank lines. */
const blocksOf = (text: string): string[] => text.split(/\n[ \t]*\n/).map((b) => b.trim()).filter(Boolean)

interface Run { from: Y.RelativePosition; to: Y.RelativePosition }

export class LocalIntent {
  private runs: Run[] = []
  private readonly watch = (event: Y.YTextEvent, tr: Y.Transaction) => {
    if (!this.mine(tr.origin)) return
    let at = 0
    for (const d of event.delta) {
      if (d.retain != null) at += d.retain
      else if (typeof d.insert === 'string') {
        this.note(at, d.insert.length)
        at += d.insert.length
      }
      // A deletion moves nothing forward and adds no text.
    }
  }

  /** `mine` says whether a transaction's origin is this person's own editor (the host knows its sync configuration and undo manager). */
  constructor(private readonly text: Y.Text, public mine: (origin: unknown) => boolean) {
    text.observe(this.watch)
  }

  /** Notes `length` characters inserted at `index`, extending the last run when this one starts where it ended. */
  private note(index: number, length: number) {
    if (!length) return
    const doc = this.text.doc!
    const last = this.runs.at(-1)
    const end = last ? Y.createAbsolutePositionFromRelativePosition(last.to, doc) : null
    // The insert has already happened: its start is `index`, and the previous run ended exactly there when typing goes on.
    const to = Y.createRelativePositionFromTypeIndex(this.text, index + length, -1)
    if (last && end && end.index === index) last.to = to
    else this.runs.push({ from: Y.createRelativePositionFromTypeIndex(this.text, index, 0), to })
  }

  /** The room confirmed this person's edits: nothing is owed. */
  clear(): void { this.runs = [] }

  /** What is owed: this person's own unconfirmed insertions as Markdown blocks (empty when there are none). */
  blocks(): string[] {
    const doc = this.text.doc
    if (!doc) return []
    const whole = this.text.toString()
    const spans: Array<[number, number]> = []
    for (const r of this.runs) {
      const a = Y.createAbsolutePositionFromRelativePosition(r.from, doc)
      const b = Y.createAbsolutePositionFromRelativePosition(r.to, doc)
      if (a && b && b.index > a.index) spans.push([a.index, b.index])
    }
    spans.sort((x, y) => x[0] - y[0])
    // A run that a peer typed inside of is cut around the peer's words: the runs' own characters are those whose item was inserted by this editor, which is what the per-transaction runs already isolate; adjacent spans join into one piece.
    const pieces: string[] = []
    let cur = ''
    let curEnd = -1
    for (const [a, b] of spans) {
      if (a === curEnd) { cur += whole.slice(a, b); curEnd = b }
      else { if (cur) pieces.push(cur); cur = whole.slice(a, b); curEnd = b }
    }
    if (cur) pieces.push(cur)
    return blocksOf(pieces.join('\n\n'))
  }

  destroy(): void { this.text.unobserve(this.watch) }
}
