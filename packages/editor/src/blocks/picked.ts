/**
 * How a block looks once a touch has picked it (#397). On an iPhone the editor has no selection
 * paint of its own (the phone paints none with the keyboard down, and CodeMirror adds only two
 * handles), so a picked block is drawn here, as kayg approved it:
 *
 * - while a block is picked, every block slides right to make room, and a 24 pt circle shows in
 *   the left gutter: a hollow ring on the blocks that aren't picked, a filled accent circle with
 *   a white check on the ones that are. The slide is a critically damped spring, no overshoot;
 *   it slides back when the pick ends;
 * - a picked block wears a rounded tint, 8 px past its text;
 * - no handles and no native selection paint while it lasts (`quiet` below: the phone's own paint
 *   ignores `::selection`, so the page's selection is parked as a caret while it lasts).
 *
 * This file and follow.ts are the whole look: a different look replaces them, not touch.ts.
 * Picking lasts while the selection is what `pickBlock` was given; any other selection or any
 * change to the note ends it.
 */

import { EditorSelection, EditorState, type Extension, StateEffect, StateField } from '@codemirror/state'
import { EditorView, type LayerMarker, RectangleMarker, ViewPlugin, layer } from '@codemirror/view'
import { type Block, blockAt } from './blocks'
import { blockBox, nudged, watchLayout } from './box'
import { type Spring, run } from './spring'

interface Span {
  from: number
  to: number
}

/** The circle, and the gap between it and the text. */
export const SIZE = 24
const GAP = 8
/** At least how far a block slides to leave room for its circle. */
const SHIFT = SIZE + GAP
/** Where the text starts once the blocks have slid, from the scroller's edge (a page margin of 20 slides 40). */
const TEXT_AT = 60

/** How far the blocks slide: enough to put the text at TEXT_AT, never less than the circle and its gap. */
function shiftFor(view: EditorView): number {
  const lead = view.contentDOM.getBoundingClientRect().left - view.scrollDOM.getBoundingClientRect().left
  return Math.max(SHIFT, TEXT_AT - lead)
}
/** The tint reaches this far past the text on each side. */
const PAST = 8
/** The circle's left edge, from the scroller's edge. The tint starts after it, never under it. */
export const CIRCLE_LEFT = 14
/** The tint keeps this far from the right edge (once the blocks have slid). */
const RIGHT_INSET = 20
const setPicked = StateEffect.define<Span[] | null>()

const hull = (spans: Span[]): Span => ({ from: Math.min(...spans.map((s) => s.from)), to: Math.max(...spans.map((s) => s.to)) })

/** Where the editor's selection is parked, as a caret, while blocks are picked (see `quiet`), or null. */
const setPark = StateEffect.define<number>()
/** The user event of the park's own step (not `select…`: a hold would turn that back). */
const PARK = 'park'
const park = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setPark)) return e.value
      if (e.is(setPicked) && e.value == null) return null
    }
    if (value == null || tr.docChanged) return null
    // Only the park itself (or the same selection again, as a hold turns one back) keeps it: a tap
    // on the very spot is the person's, and ends it.
    if (tr.selection && !tr.newSelection.eq(tr.startState.selection) && !tr.isUserEvent(PARK)) return null
    return value
  },
})

const picked = StateField.define<Span[] | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setPicked)) return e.value
    if (!value) return null
    if (tr.docChanged) return null
    if (tr.selection) {
      // The selection is the hull of the picked blocks (the app keeps it so, whatever lies between).
      const s = tr.newSelection.main
      const h = hull(value)
      // The hull, or the caret the pick is parked as (it came with the selection, in the same step).
      const kept = tr.newSelection.eq(tr.startState.selection) || (s.from === h.from && s.to === h.to)
      if (!kept && !(tr.isUserEvent(PARK) && s.empty && s.head === tr.state.field(park))) return null
    }
    return value
  },
})

/** Where a block sits on screen (box.ts): its top and bottom as the tint draws them, and its first row's middle. */
export function boxOf(view: EditorView, block: Block) {
  return blockBox(view, block)
}

/** The scroller's origin, so page coordinates become positions inside it. */
const origin = (view: EditorView) => {
  const r = view.scrollDOM.getBoundingClientRect()
  return { x: r.left - view.scrollDOM.scrollLeft, y: r.top - view.scrollDOM.scrollTop }
}

/** The blocks with a line on screen, from the top. */
function visibleBlocks(view: EditorView): Block[] {
  const out: Block[] = []
  const { doc } = view.state
  let n = doc.lineAt(view.viewport.from).number
  const end = doc.lineAt(view.viewport.to).number
  while (n <= end) {
    const block = blockAt(view.state, doc.line(n).from)
    if (!block) {
      n++
      continue
    }
    out.push(block)
    n = Math.max(doc.lineAt(block.to).number, n) + 1
  }
  return out
}

const changed = (u: import('@codemirror/view').ViewUpdate) =>
  u.docChanged || u.viewportChanged || u.geometryChanged || u.selectionSet || nudged(u) || u.transactions.some((t) => t.effects.some((e) => e.is(setPicked)))

const isPicked = (spans: Span[], b: Block) => spans.some((s) => b.from < s.to && b.to > s.from)

/** The tint: behind the text, and slid with the blocks (`--pick-x`, set by `slide` below). */
const tintLayer = layer({
  above: false,
  class: 'cm-picked-tint-layer',
  markers(view) {
    const span = view.state.field(picked)
    if (!span) return []
    const o = origin(view)
    const content = view.contentDOM.getBoundingClientRect()
    const room = Number.parseFloat(view.dom.style.getPropertyValue('--pick-room')) || 0
    const out: LayerMarker[] = []
    for (const b of visibleBlocks(view)) {
      if (!isPicked(span, b)) continue
      const box = boxOf(view, b)
      if (!box) continue
      out.push(new RectangleMarker('cm-picked-bg', content.left - PAST - o.x, box.top - o.y, content.width - room + 2 * PAST, box.bottom - box.top))
    }
    return out
  },
  update: changed,
})

/** The circles, in the left gutter: they stay put while the blocks slide. */
const circleLayer = layer({
  above: true,
  class: 'cm-picked-circles',
  markers(view) {
    const span = view.state.field(picked)
    if (!span) return []
    const o = origin(view)
    const left = view.scrollDOM.getBoundingClientRect().left + CIRCLE_LEFT - o.x
    const out: LayerMarker[] = []
    for (const b of visibleBlocks(view)) {
      const box = boxOf(view, b)
      if (!box) continue
      out.push(new RectangleMarker(isPicked(span, b) ? 'cm-pick-ring cm-pick-ring-on' : 'cm-pick-ring', left, box.mid - SIZE / 2 - o.y, SIZE, SIZE))
    }
    return out
  },
  update: changed,
})

/** Slides every block right while a block is picked, on one spring, through `--pick-x`. */
const slide = ViewPlugin.define((view) => {
  const s: Spring = { pos: 0, vel: 0 }
  let stop = () => {}
  let want = 0
  const draw = (x: number) => {
    view.dom.style.setProperty('--pick-x', `${x}px`)
  }
  const done = () => {
    if (want === 0) {
      delete view.dom.dataset.sliding
      view.dom.style.removeProperty('--pick-room')
    }
  }
  return {
    update(u) {
      const on = u.state.field(picked) != null
      const next = on ? shiftFor(view) : 0
      if (next === want) return
      want = next
      stop()
      if (on) {
        // Text keeps its right edge inside the page: it narrows by what the slide would push out.
        const pad = view.scrollDOM.getBoundingClientRect().right - view.contentDOM.getBoundingClientRect().right
        view.dom.style.setProperty('--pick-room', `${Math.max(0, next + PAST + RIGHT_INSET - pad)}px`)
        view.dom.dataset.sliding = ''
      }
      if (matchMedia('(prefers-reduced-motion: reduce)').matches) {
        s.pos = want
        s.vel = 0
        draw(want)
        done()
        return
      }
      stop = run(s, () => want, draw, done)
    },
    destroy: () => stop(),
  }
})

/**
 * Keeps the phone from painting the picked words as selected text (#397). With the keyboard up, iOS
 * paints the page's selection itself (darker, ragged rectangles behind each line, and its handles;
 * UIKit draws them, so no `::selection` style reaches them) and the editor keeps the page's
 * selection equal to its own. So while blocks are picked and the editor has the keyboard, the
 * editor's selection is a caret (where the selection's head was): it paints nothing and keeps the
 * keyboard up. The pick lives on in `picked`, and in the app's own block list, which every block
 * action reads. Any other selection (a tap, a key) or any edit ends the pick as before.
 */
const quiet = ViewPlugin.define((view) => {
  /** The editor has the keyboard (`view.hasFocus` also asks the window, which a test browser may not have). */
  const typing = () => view.root.activeElement === view.contentDOM
  /** The editor had the keyboard when the pick began. If it comes only later (a tap on the words), that tap is the person's. */
  let had = false
  const hush = () => {
    const { main } = view.state.selection
    if (!had || !view.state.field(picked) || !typing() || (main.empty && view.state.field(park) === main.head)) return
    view.dispatch({ selection: EditorSelection.cursor(main.head), effects: setPark.of(main.head), userEvent: PARK })
  }
  let frame = 0
  return {
    update(u) {
      const was = u.startState.field(picked) != null
      const on = u.state.field(picked) != null
      if (on && !was) had = typing()
      else if (!on) had = false
      // After the update has put the editor's selection on the page, before the next paint.
      if (on && !frame)
        frame = requestAnimationFrame(() => {
          frame = 0
          hush()
        })
    },
    destroy: () => cancelAnimationFrame(frame),
  }
})

/** Is the editor's selection a caret that only hides a picked block's text selection (see `quiet`)? */
export const parked = (state: EditorState): boolean => state.field(park, false) != null

/** The slide so far, for a swipe that has to stay inside the page. */
export const slidBy = (view: EditorView): number => Number.parseFloat(view.dom.style.getPropertyValue('--pick-x')) || 0

const accent = 'var(--accent, #b45309)'
const CHECK = `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='3' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M5 12.5l4.5 4.5L19 7.5'/%3E%3C/svg%3E") center / 100% no-repeat`

const theme = EditorView.baseTheme({
  '&.cm-picking .cm-selectionHandle': { display: 'none' },
  '&.cm-picking .cm-selectionBackground': { background: 'transparent !important' },
  '&.cm-picking .cm-content ::selection': { background: 'transparent !important' },
  '&.cm-picking .cm-content': { caretColor: 'transparent !important' },
  '&.cm-picking .cm-cursor, &.cm-picking .cm-fat-cursor': { display: 'none' },
  '&[data-sliding] .cm-line': { translate: 'var(--pick-x, 0px) 0', marginRight: 'var(--pick-room, 0px)' },
  '& .cm-picked-tint-layer': { translate: 'var(--pick-x, 0px) 0' },
  '& .cm-picked-bg, & .cm-pull-tint': {
    background: `color-mix(in oklab, ${accent} 16%, transparent)`,
    borderRadius: '10px',
  },
  '& .cm-pick-ring': {
    boxSizing: 'border-box',
    display: 'grid',
    placeItems: 'center',
    borderRadius: '50%',
    border: '2px solid color-mix(in oklab, var(--ink, currentColor) 38%, transparent)',
    pointerEvents: 'none',
    animation: 'cm-ring-in 250ms ease-out',
  },
  '& .cm-pick-ring-on': { background: accent, borderColor: accent },
  // The check, as the app's icon draws it (16 px, stroke 3 on a 24 grid), in the accent's ink.
  '& .cm-pick-ring-on::after': {
    content: '""',
    width: '16px',
    height: '16px',
    background: 'var(--accent-ink, #fff)',
    mask: CHECK,
    WebkitMask: CHECK,
  },
  '@keyframes cm-ring-in': { from: { opacity: 0 }, to: { opacity: 1 } },
  '@media (prefers-reduced-motion: reduce)': { '& .cm-pick-ring': { animation: 'none' } },
})

/** While a block is picked, the editor says so, and the phone's own selection paint steps aside. */
const pickingClass = EditorView.editorAttributes.compute([picked], (state) => ({ class: state.field(picked) ? 'cm-picking' : '' }))

/** Gives these blocks the picked look; call it right after selecting them. */
export function pickBlock(view: EditorView, ...spans: Span[]): void {
  view.dispatch({ effects: setPicked.of(spans.map((s) => ({ from: s.from, to: s.to }))) })
}

/** Is a block pick on screen? Then a swipe on another block adds it, or takes it out. */
export const isPicking = (view: EditorView): boolean => view.state.field(picked) != null

/** Adds the block to the pick, or takes it out; call it before the app moves the selection. */
export function togglePick(view: EditorView, block: Span): void {
  const now = view.state.field(picked) ?? []
  const has = now.some((s) => s.from === block.from)
  const next = has ? now.filter((s) => s.from !== block.from) : [...now, { from: block.from, to: block.to }].sort((a, b) => a.from - b.from)
  view.dispatch({ effects: setPicked.of(next.length ? next : null) })
}

export const pickedBlock: Extension = [park, picked, tintLayer, circleLayer, slide, quiet, pickingClass, theme, watchLayout((state) => state.field(picked) != null)]
