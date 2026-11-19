/**
 * Hidden blocks (ADR 0056, #611): the blocks listed under `hidden blocks:` in the frontmatter leave
 * the reading view. Nothing is drawn in their place (no placeholder line) and nothing is removed
 * from the file. Peeking brings them back faded: all of them ("Show hidden"), or only the one a
 * direct link names, for that visit. The editor is built again for each note, so a peek never
 * outlives the note.
 *
 * Cost on the typing path: a note without hidden blocks pays nothing. One with them looks at the
 * changed lines only, and draws again only when a change touches the frontmatter or a marker line.
 */

import { type EditorState, Facet, type Range, RangeSet, StateEffect, StateField, type Transaction } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, WidgetType } from '@codemirror/view'
import { frontmatterEnd } from '../syntax/frontmatter'
import { blockAt } from './blocks'
import { hiddenRefs } from './hidden-refs'

export interface Peek {
  /** Every hidden block shows, faded. */
  all: boolean
  /** One hidden block shows, faded (a link to it). */
  only: string | null
}

interface Value {
  /** Drawn at all: not in source mode, which shows the file as it is. */
  active: boolean
  ids: string[]
  peek: Peek
  deco: DecorationSet
}

/** Show or put away hidden blocks. */
export const setPeek = StateEffect.define<Partial<Peek>>()

/** Draw hidden blocks (reading view) or not (source mode). */
const setActive = StateEffect.define<boolean>()

const MARKER = / \^([A-Za-z0-9-]{2,})\s*$/
const none: Peek = { all: false, only: null }
/** The app's answer to the unhide button. */
const unhideWith = Facet.define<(id: string) => void, ((id: string) => void) | null>({ combine: (v) => v[0] ?? null })

class UnhideWidget extends WidgetType {
  constructor(readonly id: string) {
    super()
  }
  override eq(other: UnhideWidget): boolean {
    return other.id === this.id
  }
  toDOM(view: EditorView): HTMLElement {
    const b = document.createElement('button')
    b.type = 'button'
    b.className = 'cm-unhide bounce'
    b.tabIndex = -1
    b.contentEditable = 'false'
    b.setAttribute('aria-label', 'Unhide')
    b.dataset.id = this.id
    // The eye, drawn as the app's icons are (24 box, round strokes, the ink's colour).
    const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
    svg.setAttribute('viewBox', '0 0 24 24')
    svg.setAttribute('aria-hidden', 'true')
    const eye = document.createElementNS('http://www.w3.org/2000/svg', 'path')
    eye.setAttribute('d', EYE)
    svg.append(eye)
    b.append(svg)
    b.addEventListener('mousedown', (e) => e.preventDefault())
    b.addEventListener('click', () => view.state.facet(unhideWith)?.(this.id))
    return b
  }
  override ignoreEvent(): boolean {
    return true
  }
}

const EYE = 'M2.5 12C3.4 10 7 6 12 6s8.6 4 9.5 6c-.9 2-4.5 6-9.5 6s-8.6-4-9.5-6zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z'
const fade = Decoration.line({ class: 'cm-hidden-block' })

function idsOf(state: EditorState): string[] {
  const end = frontmatterEnd(state)
  return end ? hiddenRefs(state.doc.sliceString(0, end)) : []
}

function build(state: EditorState, ids: string[], peek: Peek): DecorationSet {
  if (!ids.length) return Decoration.none
  const doc = state.doc
  const wanted = new Set(ids)
  const out: Range<Decoration>[] = []
  const body = doc.lineAt(frontmatterEnd(state)).number
  for (let n = Math.min(doc.lines, body); n <= doc.lines; n++) {
    const line = doc.line(n)
    const id = MARKER.exec(line.text)?.[1]
    if (!id || !wanted.has(id) || line.from < frontmatterEnd(state)) continue
    const block = blockAt(state, line.from)
    if (!block) continue
    const first = doc.lineAt(block.from)
    const last = doc.lineAt(block.to)
    if (peek.all || peek.only === id) {
      for (let l = first.number; l <= last.number; l++) out.push(fade.range(doc.line(l).from))
      // The unhide button sits at the block's middle line, so it is centred on the block (the mockup's place).
      const middle = doc.line(first.number + ((last.number - first.number) >> 1))
      out.push(Decoration.widget({ widget: new UnhideWidget(id), side: 1 }).range(middle.to))
    } else {
      // The block's lines go, and one blank line beside them, so the gap closes too.
      let from = first.from
      let to = last.to
      if (last.number < doc.lines && doc.line(last.number + 1).text.trim() === '') to = doc.line(last.number + 1).to
      else if (first.number > 1 && doc.line(first.number - 1).text.trim() === '') from = doc.line(first.number - 1).from
      out.push(Decoration.replace({ block: true }).range(from, to))
    }
    n = last.number
  }
  return Decoration.set(out, true)
}

/** Whether a change touched a line that holds a marker, before or after it. */
function touchesMarker(tr: Transaction): boolean {
  let hit = false
  tr.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
    if (hit) return
    const before = tr.startState.doc
    const after = tr.state.doc
    const a = before.sliceString(before.lineAt(fromA).from, before.lineAt(toA).to)
    const b = after.sliceString(after.lineAt(fromB).from, after.lineAt(toB).to)
    hit = a.includes('^') || b.includes('^')
  })
  return hit
}

const field = StateField.define<Value>({
  create(state) {
    const ids = idsOf(state)
    return { active: true, ids, peek: none, deco: build(state, ids, none) }
  },
  update(value, tr) {
    let { active, peek } = value
    for (const e of tr.effects) {
      if (e.is(setPeek)) peek = { ...peek, ...e.value }
      else if (e.is(setActive)) active = e.value
    }
    if (peek !== value.peek) return { active, ids: value.ids, peek, deco: build(tr.state, value.ids, peek) }
    if (active !== value.active) return { ...value, active }
    if (!tr.docChanged) return value
    if (tr.changes.touchesRange(0, frontmatterEnd(tr.startState))) {
      const ids = idsOf(tr.state)
      return { active, ids, peek, deco: build(tr.state, ids, peek) }
    }
    if (!value.ids.length) return value
    if (touchesMarker(tr)) return { active, ids: value.ids, peek, deco: build(tr.state, value.ids, peek) }
    return { active, ids: value.ids, peek, deco: value.deco.map(tr.changes) }
  },
  provide: (f) => [
    // Source mode shows the file as it is: nothing is hidden or faded there. (Block decorations must come
    // from the state, never from a function of the view, so the mode is kept in the field: see `tracker`.)
    EditorView.decorations.from(f, (v) => (v.active ? v.deco : Decoration.none)),
    EditorView.atomicRanges.of((view) => (view.state.field(f).active ? view.state.field(f).deco : RangeSet.empty)),
  ],
})

export const hiddenBlocks = field

export interface HiddenOptions {
  /** The faded block's unhide button was pressed. */
  onUnhide: (id: string) => void
  /** How many blocks the note hides and whether they are showing, whenever either changes (0, false when the editor goes). */
  onState: (count: number, peeking: boolean) => void
}

/** Reports the count and the peek to the app, on change only. */
function tracker(report: HiddenOptions['onState']) {
  return ViewPlugin.define((view) => {
    let count = -1
    let peeking = false
    // The editor sets `data-mode` and then reconfigures (syntax/mode.ts): follow it, a moment later, outside the update.
    const follow = () => {
      const on = view.dom.dataset.mode !== 'source'
      if (view.state.field(field).active !== on) view.dispatch({ effects: setActive.of(on) })
    }
    queueMicrotask(follow)
    const look = () => {
      const v = view.state.field(field)
      if (v.ids.length !== count || v.peek.all !== peeking) report((count = v.ids.length), (peeking = v.peek.all))
    }
    look()
    return {
      update: (u) => {
        if (u.transactions.some((t) => t.reconfigured)) queueMicrotask(follow)
        if (u.docChanged || u.transactions.some((t) => t.effects.length)) look()
      },
      destroy: () => report(0, false),
    }
  })
}

/** Everything hidden blocks add to the editor: a lazy piece the app fetches only for a note that has them. */
export const hiddenExtension = (opts: HiddenOptions) => [field, unhideWith.of(opts.onUnhide), tracker(opts.onState)]

/** Which blocks are hidden by the note (ids without the `^`). */
export const hiddenBlockList = (state: EditorState): string[] => state.field(field, false)?.ids ?? []

/** What is peeking now. */
export const hiddenPeek = (state: EditorState): Peek => state.field(field, false)?.peek ?? none

/** Whether the hidden-blocks piece is in this editor. */
export const hiddenInstalled = (state: EditorState): boolean => state.field(field, false) !== undefined
