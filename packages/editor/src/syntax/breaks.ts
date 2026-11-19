/**
 * Breaks: a `---` line (a thematic break) in the default mode.
 *
 * The line gets the `cm-break` class and the app draws it: a divider on a
 * full page, the gap between two cards on a floating one. The `---` itself
 * is hidden, except while the caret is on that line: then it shows, muted
 * (`cm-break-active`), so you can see what you are about to delete. The
 * app gives both states the same height, so moving the caret never moves
 * text.
 *
 * While the caret rests on a break's line, the editor carries the class `cm-caret-gap`, so a floating page (where that line is the empty gap between two cards) can
 * draw no caret there: a caret in the backdrop would look like text in nothing.
 *
 * Typing `---` on its own line makes the break at once (see `typeRule`).
 * The file still says `---`.
 */

import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import type { EditorState, Range } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import { noteUndo } from '../undo'
import { frontmatterEnd } from './frontmatter'

const hide = Decoration.replace({})
const line = Decoration.line({ class: 'cm-break' })
const activeLine = Decoration.line({ class: 'cm-break cm-break-active' })
const mark = Decoration.mark({ class: 'cm-break-mark' })
const lone = Decoration.line({ class: 'cm-lone' })

/**
 * A break: the line it sits on and its `---`. `num` (the line's number) and `end` (where the line ends) are read once, when the break is found, from the line lookup that
 * finds `line` anyway, so drawing after a key press or a caret move needs no line lookup per break.
 */
interface Break {
  line: number
  num: number
  end: number
  from: number
  to: number
}

/** The breaks in the drawn part of the note. */
function findBreaks(view: EditorView): Break[] {
  const { state } = view
  const body = frontmatterEnd(state)
  const tree = ensureSyntaxTree(state, view.viewport.to, 10_000) ?? syntaxTree(state)
  const out: Break[] = []
  tree.iterate({
    from: Math.max(view.viewport.from, body),
    to: view.viewport.to,
    enter: (n) => {
      if (n.name !== 'HorizontalRule') return
      if (n.from < body) return false
      const ln = state.doc.lineAt(n.from)
      out.push({ line: ln.from, num: ln.number, end: ln.to, from: n.from, to: n.to })
      return false
    },
  })
  return out
}

/** The caret (or a selection) touches this break's line. */
function touched(state: EditorState, b: Break): boolean {
  return state.selection.ranges.some((r) => r.from <= b.end && r.to >= b.line)
}

class BreaksView {
  breaks: Break[]
  decorations: DecorationSet = Decoration.none
  atomic: DecorationSet = Decoration.none
  /** True while the caret (an empty main selection) is on a break's line; read by `caretInGap`. Set by `draw`, so no extra work per key press. */
  inGap = false

  constructor(view: EditorView) {
    this.breaks = findBreaks(view)
    this.draw(view.state)
  }

  update(u: ViewUpdate) {
    const found = u.docChanged || u.viewportChanged || syntaxTree(u.state) !== syntaxTree(u.startState)
    if (found) this.breaks = findBreaks(u.view)
    // Moving the caret only redraws from the list: no parsing, no tree walk.
    if (found || u.selectionSet) this.draw(u.state)
  }

  private draw(state: EditorState) {
    const deco: Range<Decoration>[] = []
    const atomic: Range<Decoration>[] = []
    const main = state.selection.main
    let inGap = false
    this.markLone(state, deco)
    for (const b of this.breaks) {
      if (touched(state, b)) {
        deco.push(activeLine.range(b.line), mark.range(b.from, b.to))
        if (main.empty && main.head >= b.line && main.head <= b.end) inGap = true
      } else {
        deco.push(line.range(b.line), hide.range(b.from, b.to))
        atomic.push(hide.range(b.from, b.to))
      }
    }
    this.decorations = Decoration.set(deco, true)
    this.atomic = Decoration.set(atomic, true)
    this.inGap = inGap
  }

  /**
   * A card of ONE line (between two breaks) gets the class `cm-lone` on that line, so the app can give it the room a card that opens with a heading has (a squat card, its line on the
   * bottom edge). Only the short gaps are looked at: break, an optional blank line, the line, an optional blank line, break. The gap's size comes from the line numbers the breaks
   * already carry, so a pair of long cards costs a subtraction and no line lookup; only a pair with a gap of 2 to 4 lines reads those lines. No stylesheet has to look at a line's
   * neighbours (a `:has()` for it measured slower while typing).
   */
  private markLone(state: EditorState, out: Range<Decoration>[]) {
    const doc = state.doc
    for (let i = 0; i + 1 < this.breaks.length; i++) {
      const a = this.breaks[i]!.num
      const b = this.breaks[i + 1]!.num
      if (b - a < 2 || b - a > 4) continue
      let text = 0
      let at = 0
      for (let n = a + 1; n < b; n++) {
        if (doc.line(n).text.trim() === '') continue
        text++
        at = n
      }
      if (text === 1) out.push(lone.range(doc.line(at).from))
    }
  }
}

const breaksPlugin = ViewPlugin.fromClass(BreaksView, {
  decorations: (v) => v.decorations,
  provide: (p) => EditorView.atomicRanges.of((view) => view.plugin(p)?.atomic ?? Decoration.none),
})

/**
 * The class `cm-caret-gap` on the editor while the caret (an empty selection) is on a break's line. The breaks plugin already knows which break lines the selection touches
 * (it draws them), so this only reads its flag: a `---` inside code or a table is not a break and never counts, and a key press does no extra scan or state update for it.
 */
const GAP_ON: Record<string, string> = { class: 'cm-caret-gap' }
const GAP_OFF: Record<string, string> = {}
const caretInGap = EditorView.editorAttributes.of((view) => (view.plugin(breaksPlugin)?.inGap ? GAP_ON : GAP_OFF))

/** Where each break line starts, in the drawn part of the note (none in source mode). */
export function breakLines(view: EditorView): number[] {
  return view.plugin(breaksPlugin)?.breaks.map((b) => b.line) ?? []
}

/** Where a lone `---` line is not a break: code, tables and raw HTML keep their dashes as typed. */
const KEEPS_DASHES = /^(FencedCode|CodeBlock|Table|HTMLBlock|CommentBlock)$/

/**
 * Typing the third `-` of a lone `---` line makes it a break at once: the dash goes in, then the caret
 * moves to the line below (a new one, or the blank one already there), so the divider is drawn
 * without a Return. The two steps are two undo steps, so Undo (the first one) brings back the plain
 * `---` with the caret after it.
 *
 * It stays out of the way of:
 *  - the first line (`---` there opens the properties, ADR 0025) and an unclosed `---` at the top,
 *    which would turn into properties once a second `---` line follows;
 *  - the properties themselves, code blocks, tables and raw HTML;
 *  - a line that holds anything else (`--- text`, `----`): only a line that is exactly `---` converts.
 *
 * Typed straight under a line of text, `---` would make that text a heading (a "setext" heading),
 * so a blank line goes in first; that keeps it a break in every markdown reader. The file says `---`.
 */
const typeRule = EditorView.inputHandler.of((view, from, to, text) => {
  const { state } = view
  const body = frontmatterEnd(state)
  if (text !== '-' || from < body || (!body && state.doc.line(1).text.trimEnd() === '---')) return false
  const next = state.update({ changes: { from, to, insert: text } }).state
  const ln = next.doc.lineAt(from)
  if (ln.number < 2 || ln.text !== '---' || from + 1 !== ln.to) return false
  const tree = ensureSyntaxTree(next, ln.to, 50) ?? syntaxTree(next)
  let gap = 0
  for (let n: ReturnType<typeof tree.resolveInner> | null = tree.resolveInner(ln.from, 1); n; n = n.parent) {
    if (KEEPS_DASHES.test(n.name)) return false
    if (n.name === 'SetextHeading2') gap = 1
  }
  const end = from + gap + 1
  const undo = state.facet(noteUndo)
  view.dispatch({
    changes: gap ? [{ from: ln.from, insert: '\n' }, { from, to, insert: text }] : { from, to, insert: text },
    selection: { anchor: end },
    userEvent: 'input.type',
  })
  undo?.stopCapturing()
  // A blank line below takes the caret; otherwise a new one is made.
  view.dispatch(
    /^\n\n?$/.test(view.state.sliceDoc(end, end + 2))
      ? { selection: { anchor: end + 1 } }
      : { changes: { from: end, insert: '\n' }, selection: { anchor: end + 1 }, userEvent: 'input.type' },
  )
  undo?.stopCapturing()
  return true
})

/** Breaks for the default mode. */
export const breaks = [breaksPlugin, typeRule, caretInGap]
