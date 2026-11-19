/**
 * A swipe you can see (#397): while a finger swipes across a block, the block follows it
 * sideways (half the distance, at most 56 px, and never past the page's edge), a ring in the
 * gutter fills and the block is tinted once the swipe is far enough to select, and when the
 * finger lifts the block settles back on the same critically damped spring as the slide
 * (spring.ts), with no overshoot.
 *
 * Only the lines' own `transform` is touched, never the text, so nothing is saved or undone.
 */

import type { EditorView } from '@codemirror/view'
import type { Block } from './blocks'
import { CIRCLE_LEFT, SIZE, boxOf, slidBy } from './picked'
import { type Spring, run } from './spring'

/** The finger's travel that moves the block by one pixel is this many pixels. */
const GIVE = 0.5
/** The most the block moves from where it was. */
const MOST = 56
/** The block stays this far inside the page's edge. */
const EDGE = 4

let back: (() => void) | null = null

/** The `.cm-line` elements of a block's own lines. */
function linesOf(view: EditorView, block: Block): HTMLElement[] {
  const out: HTMLElement[] = []
  const { doc } = view.state
  for (let n = doc.lineAt(block.from).number; n <= doc.lineAt(block.to).number; n++) {
    const node = view.domAtPos(doc.line(n).from).node
    const el = (node instanceof Element ? node : node.parentElement)?.closest<HTMLElement>('.cm-line')
    if (el) out.push(el)
  }
  return out
}

/** How far the block shows moved for a finger that moved `dx`, kept inside the page. */
function shown(view: EditorView, dx: number): number {
  const scroller = view.scrollDOM.getBoundingClientRect()
  const content = view.contentDOM.getBoundingClientRect()
  const slid = slidBy(view)
  const room = Number.parseFloat(view.dom.style.getPropertyValue('--pick-room')) || 0
  const right = scroller.right - (content.right - room + slid) - EDGE
  const left = content.left + slid - scroller.left - EDGE
  return Math.max(-Math.min(MOST, left), Math.min(MOST, right, dx * GIVE))
}

/** One element in the scroller, made on first use. */
function piece(view: EditorView, cls: string, z: string): HTMLElement {
  // Found by its own class: the last one, since a ring also wears the circles' shared class.
  let el = view.scrollDOM.querySelector<HTMLElement>(`.${cls.split(' ').at(-1)}`)
  if (!el) {
    el = document.createElement('div')
    el.className = cls
    el.style.cssText = `position:absolute;pointer-events:none;z-index:${z}`
    view.scrollDOM.appendChild(el)
  }
  return el
}

/** The ring and the tint for the block being pulled: they show once the swipe would select. */
function marks(view: EditorView, block: Block, x: number, ready: boolean): void {
  const base = view.scrollDOM.getBoundingClientRect()
  const content = view.contentDOM.getBoundingClientRect()
  const box = boxOf(view, block)
  if (!box) return
  const left = content.left - base.left + view.scrollDOM.scrollLeft
  const top = box.top - base.top + view.scrollDOM.scrollTop
  const tint = piece(view, 'cm-pull-tint', '-1')
  tint.style.cssText += `;left:${left - 8}px;top:${top}px;width:${content.width + 16}px;height:${box.bottom - box.top}px;translate:${x}px 0;opacity:${ready ? 1 : 0}`
  const ring = piece(view, 'cm-pick-ring cm-pull-ring', '1')
  ring.style.cssText += `;left:${CIRCLE_LEFT + view.scrollDOM.scrollLeft}px;top:${box.mid - base.top + view.scrollDOM.scrollTop - SIZE / 2}px;width:${SIZE}px;height:${SIZE}px`
  ring.classList.toggle('cm-pick-ring-on', ready)
}

/** Moves the block with the finger; `ready` is true once the swipe is far enough to select. */
export function follow(view: EditorView, block: Block, dx: number, ready: boolean): void {
  back?.()
  back = null
  const x = shown(view, dx)
  for (const el of linesOf(view, block)) el.style.transform = `translateX(${x}px)`
  marks(view, block, x + slidBy(view), ready)
}

/** Settles the block back from where `follow` left it, and clears what `follow` set. */
export function springBack(view: EditorView, block: Block, dx: number): void {
  const s: Spring = { pos: shown(view, dx), vel: 0 }
  const lines = linesOf(view, block)
  const clear = () => {
    for (const el of lines) el.style.transform = ''
    for (const el of view.scrollDOM.querySelectorAll('.cm-pull-tint, .cm-pull-ring')) el.remove()
  }
  view.scrollDOM.querySelectorAll('.cm-pull-ring').forEach((el) => el.remove())
  const tint = view.scrollDOM.querySelector<HTMLElement>('.cm-pull-tint')
  if (tint) tint.style.opacity = '0'
  if (matchMedia('(prefers-reduced-motion: reduce)').matches || Math.abs(s.pos) < 0.5) return clear()
  back = run(
    s,
    () => 0,
    (x) => {
      for (const el of lines) el.style.transform = `translateX(${x}px)`
      if (tint) tint.style.translate = `${x + slidBy(view)}px 0`
    },
    () => {
      back = null
      clear()
    },
  )
}
