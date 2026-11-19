/**
 * Bullet-journal tree lines in the editor (#133), default mode only.
 *
 * Every nested list item gets a thin guide from its parent's marker with a
 * `├` or `└` elbow into its own marker. Nothing is written to the file and
 * no text moves: the guides are drawn by CSS from classes and custom
 * properties on each line (see `./shape.ts` and the app's tree.css).
 *
 * To give every level the same fixed step, an item's leading spaces are
 * drawn as one spacer of `depth` steps instead of as spaces (like the other
 * hidden syntax, the spacer is atomic). Its width is known before the line
 * is drawn, so nothing reflows. Wrapped lines hang under the item's text.
 *
 * Cost: one pass over the list around the drawn lines on each change, plus
 * a cached decoration per distinct shape. Lists far above or below the
 * screen are never read.
 */

import { syntaxTree } from '@codemirror/language'
import { Annotation, type EditorState, Prec, type Range, StateEffect, type Transaction } from '@codemirror/state'
import type { SyntaxNode } from '@lezer/common'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate, WidgetType } from '@codemirror/view'
import { isRemote } from '../origins'
import { frontmatterEnd } from '../syntax/frontmatter'
import { branchRange, CONTINUATION, OUTSIDE } from './branch'
import { type ListLine, type Marker, treeShape } from './shape'

/**
 * Add `treeGrow.of(true)` to a change made with the pointer (a drag that
 * nests a block, an indent button) and the new branch grows in place.
 * Keyboard changes never animate (TASTE.md), so typing, Tab and undo don't
 * carry it. Drag and drop inside the editor counts by default.
 */
export const treeGrow = Annotation.define<boolean>()

/** The line (number) the pointer is over, or null. Only a real pointer sends it, never the keyboard. */
const hoverLine = StateEffect.define<number | null>()

const ITEM = /^([ \t]*)([-*+]|\d{1,9}[.)])(?=[ \t]|$)/
const BOX = /^[ \t]+\[[ xX/-]\](?=[ \t]|$)/
const FENCE = /^[ \t]*(```|~~~)/

/** How far to look past the drawn lines for the rest of a list. */
const SCAN_LIMIT = 4000
/** How long a grown branch keeps its class: longer than the animation. */
const GROW_MS = 600

function columns(ws: string): number {
  let w = 0
  for (const ch of ws) w = ch === '\t' ? w + 4 - (w % 4) : w + 1
  return w
}

interface Classified {
  line: ListLine
  /** Leading whitespace of an item line, drawn as the spacer. */
  ws: number
}

const MORE: Classified = { line: { kind: 'more' }, ws: 0 }
const BREAK: Classified = { line: { kind: 'break' }, ws: 0 }

/** What a line is, on its own (fences are handled by the caller). */
export function classifyLine(text: string): Classified {
  const m = ITEM.exec(text)
  if (m) {
    const ws = m[1]!.length
    const marker: Marker = BOX.test(text.slice(m[0].length)) ? 'task' : /\d/.test(m[2]!) ? 'number' : 'bullet'
    return { line: { kind: 'item', indent: columns(m[1]!), marker }, ws }
  }
  return isBreak(text) ? BREAK : MORE
}

/** A line at the margin that isn't a list item ends the list. Checked cheaply, first character first. */
function isBreak(text: string): boolean {
  const c = text.charCodeAt(0)
  if (!text.length || c === 32 || c === 9) return false
  // `-`, `*`, `+` or a digit might start an item.
  if (c === 45 || c === 42 || c === 43 || (c >= 48 && c <= 57)) return !ITEM.test(text) && !!text.trim()
  return !!text.trim()
}

/** True when `pos` sits inside a code block that isn't in a list. */
function inTopLevelCode(state: EditorState, pos: number): number | null {
  for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(pos, 1); n; n = n.parent) {
    if (n.name === 'ListItem') return null
    if (n.name === 'FencedCode' || n.name === 'CodeBlock') return n.to
  }
  return null
}

export interface TreeRegion {
  /** First line number classified. */
  first: number
  lines: Classified[]
}

/** Classifies the whole list (or lists) around lines `from`..`to` (line numbers). */
export function treeRegion(state: EditorState, from: number, to: number): TreeRegion {
  const doc = state.doc
  const body = frontmatterEnd(state)
  const bodyLine = body > 0 ? doc.lineAt(Math.min(body, doc.length)).number : 1
  let first = Math.max(from, bodyLine)
  // Starting inside a top-level code block: the list (if any) starts after it.
  const codeEnd = inTopLevelCode(state, doc.line(first).from)
  if (codeEnd != null) first = Math.min(doc.lines, doc.lineAt(codeEnd).number + 1)
  for (let n = 0; first > bodyLine && n < SCAN_LIMIT; n++) {
    if (isBreak(doc.line(first - 1).text)) break
    first--
  }
  let last = Math.max(to, first)
  for (let n = 0; last < doc.lines && n < SCAN_LIMIT; n++) {
    if (isBreak(doc.line(last + 1).text)) break
    last++
  }
  const lines: Classified[] = []
  let fence = false
  for (const iter = doc.iterLines(first, last + 1); !iter.next().done; ) {
    const text = iter.value
    if (FENCE.test(text)) {
      // A fence at the margin ends the list; one indented under an item is part of it.
      if (!/^[ \t]/.test(text)) {
        lines.push(BREAK)
        fence = false
        continue
      }
      fence = !fence
      lines.push(MORE)
      continue
    }
    lines.push(fence ? MORE : classifyLine(text))
  }
  return { first, lines }
}

/** The leading spaces of a nested item, drawn as `depth` steps. */
class IndentWidget extends WidgetType {
  constructor(readonly depth: number) {
    super()
  }
  override eq(other: IndentWidget): boolean {
    return other.depth === this.depth
  }
  toDOM(): HTMLElement {
    const el = document.createElement('span')
    el.className = 'cm-tree-indent'
    el.setAttribute('aria-hidden', 'true')
    el.style.setProperty('--tree-depth', String(this.depth))
    return el
  }
}

const indents: Decoration[] = []
const indentDeco = (depth: number) => (indents[depth] ??= Decoration.replace({ widget: new IndentWidget(depth) }))

class TreeView {
  decorations: DecorationSet = Decoration.none
  atomic: DecorationSet = Decoration.none
  /** The lit branch (#247): the lines of the item that holds the caret or is under the pointer. */
  lit: DecorationSet = Decoration.none
  /** First line number of the last region drawn, and each line's depth (see branch.ts). */
  private first = 1
  private depths: number[] = []
  private hover: number | null = null
  private readonly litDecos = new Map<number, Decoration>()
  private readonly lineDecos = new Map<string, Decoration>()
  /** Depth last drawn on each nested line start, to spot a branch that just appeared. */
  private drawn = new Map<number, string>()
  /** Line starts whose branch is growing, and until when. */
  private growing = new Map<number, number>()

  constructor(view: EditorView) {
    this.build(view)
  }

  update(u: ViewUpdate) {
    const hovered = u.transactions.flatMap((t) => t.effects).filter((e) => e.is(hoverLine)).at(-1)
    if (hovered) this.hover = hovered.value as number | null
    if (!u.docChanged && !u.viewportChanged) {
      if (hovered || u.selectionSet || u.focusChanged) this.buildLit(u.view)
      return
    }
    if (u.docChanged) {
      const grow = u.transactions.some(wantsGrow)
      const now = performance.now()
      const mapped = new Map<number, number>()
      for (const [pos, until] of this.growing) if (until > now) mapped.set(u.changes.mapPos(pos), until)
      this.growing = mapped
      const before = new Map<number, string>()
      for (const [pos, shape] of this.drawn) before.set(u.changes.mapPos(pos), shape)
      this.build(u.view, grow ? { before, until: now + GROW_MS } : undefined)
    } else this.build(u.view)
    this.buildLit(u.view)
  }

  /**
   * Lights the branch of the item under the pointer, or else the one holding the caret (while the
   * editor has focus). Only the lines in view are marked; the walk is over line depths already
   * worked out, so a selection change costs next to nothing.
   */
  private buildLit(view: EditorView) {
    const doc = view.state.doc
    // Under the pointer if that's on a list item; otherwise the caret's branch.
    let range = this.hover == null ? null : branchRange(this.depths, this.hover - this.first)
    if (!range && view.hasFocus) range = branchRange(this.depths, doc.lineAt(view.state.selection.main.head).number - this.first)
    if (!range) {
      this.lit = Decoration.none
      return
    }
    let d = this.litDecos.get(range.depth)
    if (!d) this.litDecos.set(range.depth, (d = Decoration.line({ attributes: { class: 'tree-lit', style: `--tree-lit:1;--tree-hl:${range.depth}` } })))
    const { from, to } = view.viewport
    const out: Range<Decoration>[] = []
    for (let i = range.from; i < range.to; i++) {
      const l = doc.line(this.first + i)
      if (l.to < from) continue
      if (l.from > to) break
      out.push(d.range(l.from))
    }
    this.lit = Decoration.set(out)
  }

  private build(view: EditorView, grow?: { before: Map<number, string>; until: number }) {
    const { state } = view
    const doc = state.doc
    const { from, to } = view.viewport
    const region = treeRegion(state, doc.lineAt(from).number, doc.lineAt(to).number)
    const shapes = treeShape(region.lines.map((c) => c.line))
    const deco: Range<Decoration>[] = []
    const atomic: Range<Decoration>[] = []
    const drawn = new Map<number, string>()
    const now = performance.now()
    const depths: number[] = []
    for (let i = 0; i < shapes.length; i++) {
      const shape = shapes[i]
      depths.push(!shape ? OUTSIDE : shape.item ? shape.depth : CONTINUATION)
      if (!shape) continue
      const line = doc.line(region.first + i)
      if (line.to < from || line.from > to) continue
      if (shape.branch) {
        // Only a change of depth grows: a sibling turning `└` into `├` just redraws.
        const key = String(shape.depth)
        drawn.set(line.from, key)
        if (grow && grow.before.get(line.from) !== key) this.growing.set(line.from, grow.until)
      }
      const growing = (this.growing.get(line.from) ?? 0) > now
      const cls = `cm-tree ${shape.cls}${growing ? ' tree-grow' : ''}`
      const k = cls + shape.style
      let d = this.lineDecos.get(k)
      if (!d) this.lineDecos.set(k, (d = Decoration.line({ attributes: { class: cls, style: shape.style } })))
      deco.push(d.range(line.from))
      const ws = region.lines[i]!.ws
      if (shape.item && ws > 0) {
        const r = indentDeco(shape.depth).range(line.from, line.from + ws)
        deco.push(r)
        atomic.push(r)
      }
    }
    // Shapes repeat a lot; keep the cache from growing without bound.
    if (this.lineDecos.size > 2000) this.lineDecos.clear()
    this.drawn = drawn
    this.first = region.first
    this.depths = depths
    this.decorations = Decoration.set(deco, true)
    this.atomic = Decoration.set(atomic, true)
  }
}

function wantsGrow(tr: Transaction): boolean {
  if (!tr.docChanged || isRemote(tr)) return false
  return tr.annotation(treeGrow) ?? tr.isUserEvent('move')
}

const treePlugin = ViewPlugin.fromClass(TreeView, {
  decorations: (v) => v.decorations,
  provide: (p) => [
    EditorView.atomicRanges.of((view) => view.plugin(p)?.atomic ?? Decoration.none),
    EditorView.decorations.of((view) => view.plugin(p)?.lit ?? Decoration.none),
  ],
})

/**
 * The pointer lights the branch under it (desktop). A touch never does, and any key hands the
 * light back to the caret's branch. A transaction goes out only when the pointer changes line.
 */
const treeHover = EditorView.domEventHandlers({
  pointermove(e, view) {
    if (e.pointerType === 'touch') return
    const el = (e.target as HTMLElement | null)?.closest?.('.cm-line')
    const n = el ? view.state.doc.lineAt(view.posAtDOM(el)).number : null
    if (n !== lastHover.get(view)) {
      lastHover.set(view, n)
      view.dispatch({ effects: hoverLine.of(n) })
    }
  },
  mouseleave(_e, view) {
    if (lastHover.get(view) == null) return
    lastHover.set(view, null)
    view.dispatch({ effects: hoverLine.of(null) })
  },
  keydown(_e, view) {
    if (lastHover.get(view) == null) return
    lastHover.set(view, null)
    view.dispatch({ effects: hoverLine.of(null) })
  },
})
const lastHover = new WeakMap<EditorView, number | null>()

/** Structural defaults; the look (ink, step, curve) comes from the app's tree.css. */
const treeTheme = EditorView.baseTheme({
  '.cm-tree': { position: 'relative' },
  '.cm-tree-indent': {
    display: 'inline-block',
    width: 'calc(var(--tree-step, 24px) * var(--tree-depth))',
    textIndent: '0',
  },
})

/** Tree lines for nested lists: guides, `├` and `└` branches, a fixed step per level. */
export const treeLines = [treePlugin, Prec.highest(treeHover), treeTheme]
