/**
 * Viewport first, the rest at idle (plan 1c, "the editor's first paint").
 *
 * Opening a long note used to parse all of its markdown before the first frame: the embed, table and code
 * block extensions each asked for the whole syntax tree. They now draw only as far as the tree reaches,
 * and this plugin decides how far that is:
 *
 * - Before a frame is painted, the tree covers every line CodeMirror draws (its viewport, which already
 *   includes a margin above and below the screen). The hidden-syntax pass parses that far anyway; this
 *   hands the tree to the editor state in a microtask, so the state-built parts (embeds, tables, code
 *   colours) see it in the same frame and nothing pops in.
 * - The rest of the note is parsed at idle, a small slice at a time, so a jump to the end or a table far
 *   down is ready before anyone gets there. It pauses while you type.
 *
 * What it never does: change the text, or keep any copy of it. Every key still edits the note's document in
 * its own transaction; only *when* a part of the note is decorated changes. Find, selection, undo and
 * scrolling to an anchor work on the document itself, so they reach lines that are not drawn yet.
 */

import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import { type EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'

/** About 1 to 3 ms of markdown parsing on a laptop: one idle slice, small enough to sit between frames. */
const CHUNK_CHARS = 10_000
/** The most one idle slice may take, should a chunk be slow (very long lines). */
const SLICE_MS = 8
/** Parsing for drawn lines is not optional: effectively no limit, like the hidden-syntax pass. */
const DRAWN_MS = 10_000
/** No idle parsing for this long after a key or a scroll, so it never lands between two keystrokes or scroll frames. */
const QUIET_MS = 400
/**
 * A note just shown (opened, swapped in, gone live) settles first: no idle slice for this long. On Safari, which has no
 * `requestIdleCallback`, a timer would otherwise land in the middle of the open's own frames.
 */
const SETTLE_MS = 1000
/** Safari has no `requestIdleCallback`: a timer this long stands in for it. */
const FALLBACK_MS = 90

const idle: (cb: (deadline?: IdleDeadline) => void) => number =
  typeof requestIdleCallback === 'function' ? (cb) => requestIdleCallback(cb, { timeout: 1000 }) : (cb) => window.setTimeout(cb, FALLBACK_MS)
const cancelIdle: (id: number) => void = typeof cancelIdleCallback === 'function' ? cancelIdleCallback : clearTimeout

/** Hands an already-parsed (or newly parsed) tree to the state: an empty transaction makes the language field take it. */
function push(view: EditorView, upto: number, ms: number): void {
  const tree = ensureSyntaxTree(view.state, upto, ms)
  if (tree && tree !== syntaxTree(view.state)) view.dispatch({})
}

class ParseAhead {
  private queued = false
  private gone = false
  /** The idle slice asked for (-1: none), and the timer that waits out typing (-1: none). */
  private slice = -1
  private quiet = -1
  private quietTill = 0
  /** Any scroll on the page (the editor's own scroller or the page around it) holds the idle slices off for a moment. */
  private readonly onScroll = () => (this.quietTill = performance.now() + QUIET_MS)

  constructor(readonly view: EditorView) {
    this.quietTill = performance.now() + SETTLE_MS
    view.dom.ownerDocument.addEventListener('scroll', this.onScroll, { capture: true, passive: true })
    this.catchUp()
    this.later()
  }

  update(u: ViewUpdate): void {
    // Typing: the idle slices wait, so they never land between two keys (scrolling: see `onScroll`). Another note's text,
    // or the note going live (its extensions swapped in): they wait for it to settle.
    const now = performance.now()
    if (u.transactions.some((t) => t.reconfigured) || (u.docChanged && !u.transactions.some((t) => t.isUserEvent('input') || t.isUserEvent('delete'))))
      this.quietTill = Math.max(this.quietTill, now + SETTLE_MS)
    else if (u.docChanged) this.quietTill = Math.max(this.quietTill, now + QUIET_MS)
    if (u.docChanged || u.viewportChanged) this.catchUp()
    this.later()
  }

  /** Before this frame paints: the state's tree reaches the end of the drawn lines. */
  private catchUp(): void {
    if (this.queued || syntaxTree(this.view.state).length >= this.view.viewport.to) return
    this.queued = true
    // Not now: an update is running, and it may not dispatch. A microtask still runs before the frame paints.
    queueMicrotask(() => {
      this.queued = false
      const { view } = this
      if (this.gone || syntaxTree(view.state).length >= view.viewport.to) return
      push(view, view.viewport.to, DRAWN_MS)
    })
  }

  /** Asks for an idle slice while some of the note is left to parse. */
  private later(): void {
    if (this.slice >= 0 || this.quiet >= 0 || this.gone) return
    if (syntaxTree(this.view.state).length >= this.view.state.doc.length) return
    this.slice = idle((deadline) => this.step(deadline))
  }

  /** One idle slice: parses the next part of the note, then asks for the next slice. */
  private step(deadline?: IdleDeadline): void {
    this.slice = -1
    const { view } = this
    if (this.gone || !view.dom.isConnected) return
    const wait = this.quietTill - performance.now()
    const busy = view.composing || wait > 0 || (deadline != null && !deadline.didTimeout && deadline.timeRemaining() < 4)
    if (!busy) {
      const done = syntaxTree(view.state).length
      push(view, Math.min(view.state.doc.length, done + CHUNK_CHARS), SLICE_MS)
    }
    if (busy && wait > 0) this.quiet = window.setTimeout(() => ((this.quiet = -1), this.later()), wait)
    else this.later()
  }

  destroy(): void {
    this.gone = true
    this.view.dom.ownerDocument.removeEventListener('scroll', this.onScroll, { capture: true })
    if (this.slice >= 0) cancelIdle(this.slice)
    if (this.quiet >= 0) clearTimeout(this.quiet)
  }
}

/** The editor's parse schedule: drawn lines before the frame, the rest of the note at idle. */
export const parseAhead = ViewPlugin.fromClass(ParseAhead)
