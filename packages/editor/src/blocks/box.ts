/**
 * Where a block is drawn on screen (#397): the one box that the selection tint, on a phone and
 * on a desktop, and the phone's circles are laid out from.
 *
 * It is measured from what the browser drew, never from CodeMirror's height map. The map knows
 * only the lines' own heights: a block widget's margin (the row of tag chips under the title, the
 * date heading of a day) is not in it, so everything below such a widget sat that much lower on
 * screen than the map said, and a tint drawn from the map landed above its block, covering the
 * bottom of the line before it (#397). Wrapped lines, chips and tags all live inside one `.cm-line`,
 * so the line's own box is the box of all its visual rows.
 *
 * The tint reaches `EDGE` past the text above and below (not past a line's padding: a heading's
 * space above it is not part of it), so two blocks selected one under the other keep a gap.
 */

import { type EditorState, type Extension, StateEffect } from '@codemirror/state'
import { type EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'

/** The tint reaches this far above and below the text's lines, in px... */
export const EDGE = 1
/** ...but never closer than this to a line's own edge, so two blocks selected one under the other keep a gap of twice this. */
export const GAP_V = 2

/** Where a block sits, in the page's coordinates: its top and bottom, and its first row's middle. */
export interface BlockBox {
  top: number
  bottom: number
  mid: number
}

interface Span {
  from: number
  to: number
}

/** The `.cm-line` that holds `pos`, if it is drawn. */
function lineAt(view: EditorView, pos: number): HTMLElement | null {
  const line = view.state.doc.lineAt(pos)
  if (line.to < view.viewport.from || line.from > view.viewport.to) return null
  const node = view.domAtPos(line.from).node
  return (node instanceof Element ? node : node.parentElement)?.closest<HTMLElement>('.cm-line') ?? null
}

/**
 * Where a line's text starts and ends, for the tint: `EDGE` past the text, which is inside the
 * line's padding (a heading's space above it is not the heading), and at least `GAP_V` in from the
 * line's edge (a plain line's padding is only 3 px, a list item's about 2).
 */
function reach(el: HTMLElement): { top: number; bottom: number } {
  const r = el.getBoundingClientRect()
  const s = getComputedStyle(el)
  const pad = (v: string) => Math.max(GAP_V, (Number.parseFloat(v) || 0) - EDGE)
  return { top: r.top + pad(s.paddingTop), bottom: r.bottom - pad(s.paddingBottom) }
}

/**
 * The block's box, or null when none of its lines is drawn (a block that is off screen has none
 * to draw). A block that is only partly drawn (a long code block) reaches to the edge of what is.
 */
export function blockBox(view: EditorView, block: Span): BlockBox | null {
  const first = lineAt(view, block.from)
  const last = lineAt(view, block.to)
  if (!first && !last) return null
  const edge = view.scrollDOM.getBoundingClientRect()
  const t = first ? reach(first).top : edge.top
  const b = last ? reach(last).bottom : edge.bottom
  // The middle of the block's first row (a wrapped line has more rows than one).
  const row = first ? view.coordsAtPos(block.from, 1) : null
  return { top: t, bottom: b, mid: row ? (row.top + row.bottom) / 2 : t + 14 }
}

const nudge = StateEffect.define<null>()

/** Did this update ask for the tint to be measured again? (`watchLayout` does, when the page moved.) */
export const nudged = (u: ViewUpdate): boolean => u.transactions.some((t) => t.effects.some((e) => e.is(nudge)))

/**
 * While `active` says a tint is showing, measures again when the page lays out differently without
 * the text changing: the window is resized, a font arrives, lines wrap another way, the picked
 * blocks slide over and narrow. CodeMirror hears about some of these (a line's height), not all.
 * Nothing runs while no tint shows.
 */
export function watchLayout(active: (state: EditorState) => boolean): Extension {
  return ViewPlugin.define((view) => {
    let observer: ResizeObserver | null = null
    let frame = 0
    let first = true
    const fire = () => {
      if (frame) return
      frame = requestAnimationFrame(() => {
        frame = 0
        view.dispatch({ effects: nudge.of(null) })
      })
    }
    const on = () => {
      if (observer) return
      first = true
      observer = new ResizeObserver(() => {
        // Observing starts with a report of the size it already has.
        if (first) first = false
        else fire()
      })
      observer.observe(view.contentDOM)
      document.fonts?.addEventListener('loadingdone', fire)
    }
    const off = () => {
      observer?.disconnect()
      observer = null
      document.fonts?.removeEventListener('loadingdone', fire)
      cancelAnimationFrame(frame)
      frame = 0
    }
    if (active(view.state)) on()
    return {
      update(u) {
        if (active(u.state)) on()
        else off()
      },
      destroy: off,
    }
  })
}
