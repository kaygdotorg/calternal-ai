/**
 * Selecting a block by touch (#290): a phone has no hover, so no handle to grab. Instead:
 *
 * - a swipe across a block, either way, selects it;
 * - a tap on the block's edge (the margin left of the text, where the handle would be) or on its
 *   bullet selects it;
 * - a tap on the text itself types, as always (CodeMirror places the caret).
 *
 * A selecting touch is cancelled before it ends, so the caret doesn't move and the keyboard doesn't
 * come up. What selecting means is the app's: the same `onAction('drag', …)` as the handle.
 *
 * What it looks like is ours (#397): the block follows the finger and springs back (follow.ts),
 * and once picked it wears a tint and a dot, with no handles (picked.ts). On an iPhone the
 * phone paints no selection with the keyboard down, so without this a picked block showed only
 * two little handles and read as half-selected text.
 *
 * The phone wants the same finger for itself (#397): on an iPhone a sideways drag over text is also
 * how you select words, and once iOS has started that, the page can't stop it. So a swipe is
 * claimed early, on its first clearly sideways, quick move: from then on its moves are ours
 * (`preventDefault`), which stops the scroll and any of the phone's gestures that haven't started
 * yet. What isn't quick (a hold first: the phone's long-press selection), isn't sideways (a scroll),
 * or comes right after a tap (a double-tap-drag selects words) stays the phone's. If the phone takes
 * a claimed swipe anyway (`touchcancel`), the block is still selected, and swipe-hold.ts keeps the
 * phone's selection from turning it back into words.
 */

import type { EditorView } from '@codemirror/view'
import { type Block, blockAt } from './blocks'
import { follow, springBack } from './follow'
import { CIRCLE_LEFT, SIZE, isPicking, pickBlock, togglePick } from './picked'
import { QUIET, holdFor, release } from './swipe-hold'
import { canStartAt, claimsSwipe, isNotSwipe } from './swipe-start'

/** How far a finger must travel sideways, mostly sideways, to select the block. */
const SWIPE = 32
/** A touch this soon after a tap, near it, is a double-tap-drag: the phone's word selection. */
const AGAIN = 350
/** While the finger is down, hold the selection this long (swipe-hold.ts caps it). */
const WHILE_DOWN = 3000
/** The edge: this much of the margin left of the text, plus a little into it. */
const EDGE_IN = 6
const EDGE_OUT = 44

interface Start {
  x: number
  y: number
  at: number
  block: Block | null
  edge: boolean
  /** Can still become a swipe: not the phone's swipe back, a hold, a scroll or a double tap. */
  swipeable: boolean
}

export function touchSelect(view: EditorView, select: (block: Block, toggle: boolean) => void): () => void {
  let start: Start | null = null
  let claimed = false
  /** How far the finger has moved sideways since it landed, signed, while a swipe is claimed. */
  let pulled = 0
  let lastTap = { x: 0, y: 0, at: 0 }

  const blockAtPoint = (x: number, y: number) => {
    const content = view.contentDOM.getBoundingClientRect()
    const pos = view.posAtCoords({ x: Math.max(x, content.left + 1), y }, false)
    return blockAt(view.state, pos)
  }

  /** Picks the block, or, while blocks are picked, adds it to them or takes it out. */
  const choose = (block: Block) => {
    const toggle = isPicking(view)
    if (toggle) {
      togglePick(view, block) // before the app moves the selection, so the pick keeps up with it
      select(block, true)
    } else {
      select(block, false)
      pickBlock(view, block)
    }
  }

  const swiping = (on: boolean) => {
    if (on) view.scrollDOM.dataset.swiping = ''
    else delete view.scrollDOM.dataset.swiping
  }

  const down = (e: TouchEvent) => {
    claimed = false
    swiping(false)
    if (e.touches.length !== 1) return void (start = null)
    // A table scrolls sideways inside its card; a swipe across it is never a row action (T5).
    if ((e.target as Element).closest?.('.tw')) return void (start = null)
    // A new touch is a new intention: whatever the last swipe held, let go.
    release(view)
    const t = e.touches[0]!
    const content = view.contentDOM.getBoundingClientRect()
    const onBullet = (e.target as Element).closest?.('.cm-bullet, .cm-task') != null
    // While blocks are picked, the circle in the left gutter is the edge (the blocks have slid over it).
    const scroller = view.scrollDOM.getBoundingClientRect()
    const circle = t.clientX >= scroller.left + CIRCLE_LEFT - 8 && t.clientX <= scroller.left + CIRCLE_LEFT + SIZE + 12
    const edge = onBullet || (isPicking(view) ? circle : t.clientX >= content.left - EDGE_OUT && t.clientX <= content.left + EDGE_IN)
    const now = Date.now()
    const again = now - lastTap.at < AGAIN && Math.hypot(t.clientX - lastTap.x, t.clientY - lastTap.y) < 40
    start = {
      x: t.clientX,
      y: t.clientY,
      at: now,
      block: blockAtPoint(t.clientX, t.clientY),
      edge,
      swipeable: canStartAt(t.clientX) && !again,
    }
  }

  const move = (e: TouchEvent) => {
    if (!start?.block) return
    const t = e.touches[0]!
    if (claimed) {
      if (e.cancelable) e.preventDefault()
      pulled = t.clientX - start.x
      follow(view, start.block, pulled, Math.abs(pulled) > SWIPE)
      return
    }
    if (!start.swipeable) return
    const dx = Math.abs(t.clientX - start.x)
    const dy = Math.abs(t.clientY - start.y)
    const ms = Date.now() - start.at
    if (claimsSwipe(dx, dy, ms)) {
      // Ours, before the phone decides: no scroll, no text selection, from this move on.
      claimed = true
      if (e.cancelable) e.preventDefault()
      swiping(true)
      holdFor(view, WHILE_DOWN)
      pulled = t.clientX - start.x
      follow(view, start.block, pulled, false)
    } else if (isNotSwipe(dx, dy, ms)) {
      start.swipeable = false // a scroll, or a hold: the phone's
    }
  }

  const up = (e: TouchEvent) => {
    const s = start
    const wasClaimed = claimed
    start = null
    claimed = false
    swiping(false)
    if (s?.block && wasClaimed) springBack(view, s.block, pulled)
    pulled = 0
    if (!s?.block) return
    const t = e.changedTouches[0]!
    const dx = Math.abs(t.clientX - s.x)
    const dy = Math.abs(t.clientY - s.y)
    const still = Math.hypot(dx, dy) < 10
    if (still) lastTap = { x: t.clientX, y: t.clientY, at: Date.now() }
    const swiped = wasClaimed && dx > SWIPE && dx > 2 * dy
    // A tick box is the checkbox's own: it toggles, never selects.
    const onTick = (e.target as Element).closest?.('.cm-task') != null
    if (swiped || (still && s.edge && !onTick)) {
      e.preventDefault() // no click: the caret stays put, the keyboard stays down
      choose(s.block)
      if (swiped) holdFor(view, QUIET) // a selection the phone sends late can't undo it
    } else if (wasClaimed) {
      release(view)
    }
  }

  // The phone took the finger (on an iPhone: its own text selection) in the middle of a swipe
  // we'd claimed: it was still a swipe, so the block is selected, and held while the phone's
  // selection keeps following the finger.
  const cancel = () => {
    const s = start
    const wasClaimed = claimed
    start = null
    claimed = false
    swiping(false)
    if (!s?.block || !wasClaimed) return
    springBack(view, s.block, pulled)
    pulled = 0
    choose(s.block)
    holdFor(view, QUIET)
  }

  const root = view.scrollDOM
  root.addEventListener('touchstart', down, { passive: true })
  root.addEventListener('touchmove', move, { passive: false })
  root.addEventListener('touchend', up, { passive: false })
  root.addEventListener('touchcancel', cancel, { passive: true })
  return () => {
    root.removeEventListener('touchstart', down)
    root.removeEventListener('touchmove', move)
    root.removeEventListener('touchend', up)
    root.removeEventListener('touchcancel', cancel)
    swiping(false)
  }
}
