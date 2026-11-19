/**
 * Default mode: markdown syntax is never shown.
 *
 * `# `, `**`, `[`…`](<path>)`, `> `, `- ` and `[ ]` stay in the file but are
 * hidden or drawn as a bullet or checkbox (`---` lines: see breaks.ts); a
 * link's words become a chip (see `links.ts`). Hidden ranges are atomic,
 * so the cursor jumps over them and Backspace removes one as a whole (which
 * turns a heading back into text, like any rich editor).
 *
 * Decorations are built from a fully parsed tree before the first paint,
 * so a note never flashes raw syntax and then reflows.
 */

import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import { EditorSelection, EditorState, Prec, type Extension, type Range } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import type { SyntaxNode, SyntaxNodeRef, Tree } from '@lezer/common'
import { ySyncAnnotation } from 'y-codemirror.next'
import { frontmatterEnd, hiddenFrontmatter } from './frontmatter'
import { type LinkSpan, focusedLink, linkIndexAt, linkKind, linkMarks, linkState, refreshLinks } from './links'
import { BulletWidget, CheckboxWidget, taskState } from './widgets'

const hide = Decoration.replace({})
const bullet = Decoration.replace({ widget: new BulletWidget() })
const marks = {
  Emphasis: Decoration.mark({ class: 'cm-em' }),
  StrongEmphasis: Decoration.mark({ class: 'cm-strong' }),
  Strikethrough: Decoration.mark({ class: 'cm-strike' }),
  InlineCode: Decoration.mark({ class: 'cm-code' }),
} as const
const lineClass = (cls: string) => Decoration.line({ class: cls })

/** A list line's leading mark, with an optional task box after it. */
const TASK = /^(\s*)([-*+]|\d+[.)])( +)(\[([ xX/-])\]( |$))?/

export interface Hidden {
  /** Everything to draw. */
  decorations: DecorationSet
  /** The hidden and replaced parts only: the cursor skips these. */
  atomic: DecorationSet
  /** Every link, in document order; drawn as chips by the view. */
  links: LinkSpan[]
  /** A top-level table in the drawn range needs its grid and styles. */
  hasTable: boolean
}

/** Builds the decorations for `ranges` of the document (all of it by default). */
export function buildHidden(
  state: EditorState,
  ranges?: readonly { from: number; to: number }[],
  parsed?: Tree | null,
): Hidden {
  const deco: Range<Decoration>[] = []
  const atomic: Range<Decoration>[] = []
  const links: LinkSpan[] = []
  let hasTable = false
  const doc = state.doc
  const replace = (from: number, to: number, d = hide) => {
    if (to <= from) return
    deco.push(d.range(from, to))
    atomic.push(d.range(from, to))
  }
  const spaceAfter = (pos: number) => (pos < doc.length && doc.sliceString(pos, pos + 1) === ' ' ? 1 : 0)
  // Prefer the tree that was just parsed far enough (see `parseDrawn`):
  // `syntaxTree(state)` can still be the older, shorter one.
  const tree = parsed ?? syntaxTree(state)

  const enter = (node: SyntaxNodeRef): boolean | void => {
    const { name, from, to } = node
    if (/^(ATX|Setext)Heading[1-6]$/.test(name)) {
      deco.push(lineClass(`cm-h cm-h${name.at(-1)}`).range(doc.lineAt(from).from))
      return
    }
    switch (name) {
      case 'HeaderMark': {
        const line = doc.lineAt(from)
        // Closing marks (`## Title ##`) take the space before them too.
        if (from === line.from) replace(from, to + spaceAfter(to))
        else replace(from - (doc.sliceString(from - 1, from) === ' ' ? 1 : 0), to)
        return
      }
      case 'EmphasisMark':
      case 'StrikethroughMark':
        replace(from, to)
        return
      case 'Emphasis':
      case 'StrongEmphasis':
      case 'Strikethrough':
        deco.push(marks[name].range(from, to))
        return
      case 'InlineCode': {
        deco.push(marks.InlineCode.range(from, to))
        const n = node.node
        for (let c = n.firstChild; c; c = c.nextSibling) if (c.name === 'CodeMark') replace(c.from, c.to)
        return false
      }
      case 'FencedCode':
        fencedCode(from, to)
        return false
      case 'Link':
        link(node)
        return
      case 'Autolink':
        autolink(node)
        return false
      case 'Blockquote':
        for (let pos = from; pos <= to; ) {
          const line = doc.lineAt(pos)
          deco.push(lineClass('cm-quote').range(line.from))
          pos = line.to + 1
        }
        return
      case 'QuoteMark':
        replace(from, to + spaceAfter(to))
        return
      case 'ListItem':
        listItem(from)
        return
      case 'Table':
        // A table of the note's own (not one inside a list or a quote): until its grid is drawn (tables/), its lines
        // keep the height the grid will have, so nothing jumps when it arrives.
        if (node.node.parent?.name !== 'Document') return
        hasTable = true
        for (let pos = from; pos <= to; ) {
          const line = doc.lineAt(pos)
          deco.push(lineClass('cm-table-pending').range(line.from))
          pos = line.to + 1
        }
        return false
    }
  }

  function fencedCode(from: number, to: number) {
    const first = doc.lineAt(from)
    const last = doc.lineAt(to)
    for (let pos = first.from; pos <= last.to; ) {
      const line = doc.lineAt(pos)
      deco.push(lineClass('cm-codeblock').range(line.from))
      pos = line.to + 1
    }
    replace(first.from, first.to)
    if (last.number !== first.number && /^\s*(```|~~~)/.test(last.text)) replace(last.from, last.to)
  }

  function link(node: SyntaxNodeRef) {
    const n = node.node
    const kids: { name: string; from: number; to: number }[] = []
    for (let c = n.firstChild; c; c = c.nextSibling) kids.push({ name: c.name, from: c.from, to: c.to })
    const open = kids[0]
    const close = kids.find((k, i) => i > 0 && k.name === 'LinkMark' && doc.sliceString(k.from, k.to) === ']')
    const url = kids.find((k) => k.name === 'URL')
    if (!open || !close || !url) return
    replace(open.from, open.to)
    replace(close.from, node.to)
    let href = doc.sliceString(url.from, url.to)
    if (href.startsWith('<') && href.endsWith('>')) href = href.slice(1, -1)
    links.push({ from: node.from, to: node.to, textFrom: open.to, textTo: close.from, href, kind: linkKind(href) })
  }

  function autolink(node: SyntaxNodeRef) {
    const n = node.node
    for (let c = n.firstChild; c; c = c.nextSibling) {
      if (c.name === 'LinkMark') replace(c.from, c.to)
      if (c.name === 'URL') {
        const href = doc.sliceString(c.from, c.to)
        links.push({ from: node.from, to: node.to, textFrom: c.from, textTo: c.to, href, kind: linkKind(href) })
      }
    }
  }

  function listItem(from: number) {
    const line = doc.lineAt(from)
    const m = TASK.exec(line.text)
    if (!m) return
    const markFrom = line.from + m[1]!.length
    const markTo = markFrom + m[2]!.length
    const afterSpace = markTo + m[3]!.length
    if (m[4]) {
      // `- [ ] ` disappears entirely; a checkbox takes its place.
      const boxFrom = afterSpace
      const boxTo = boxFrom + 3
      const widget = Decoration.widget({ widget: new CheckboxWidget(taskState(m[5]!), boxFrom + 1), side: -1 })
      deco.push(widget.range(markFrom))
      replace(markFrom, boxTo + (m[6] === ' ' ? 1 : 0))
      deco.push(lineClass(`cm-li cm-li-task cm-task-${taskState(m[5]!)}-line`).range(line.from))
    } else if (/[-*+]/.test(m[2]!)) {
      deco.push(Decoration.widget({ widget: new BulletWidget(), side: -1 }).range(markFrom))
      replace(markFrom, afterSpace)
      deco.push(lineClass('cm-li').range(line.from))
    } else {
      deco.push(lineClass('cm-li cm-li-ordered').range(line.from))
      // Digits all as wide as each other, so the tree guide can run through the middle of every number.
      deco.push(Decoration.mark({ class: 'cm-li-num' }).range(markFrom, markTo))
    }
  }

  // Frontmatter is hidden as a whole elsewhere; never decorate inside it.
  const body = frontmatterEnd(state)
  for (const r of ranges ?? [{ from: 0, to: doc.length }]) {
    if (r.to <= body) continue
    tree.iterate({ from: Math.max(r.from, body), to: r.to, enter })
  }
  return { decorations: Decoration.set(deco, true), atomic: Decoration.set(atomic, true), links, hasTable }
}

/** The syntax tree, parsed at least up to `pos` (a line is a tiny parse). */
export function treeUpTo(state: EditorState, pos: number): Tree {
  return ensureSyntaxTree(state, pos, LINE_PARSE_LIMIT_MS) ?? syntaxTree(state)
}

const LINE_PARSE_LIMIT_MS = 50

/**
 * Decorations cover every line CodeMirror draws (its viewport), not only
 * the lines on screen: a line drawn without its decorations and decorated a
 * frame later reflows from `- item` to `• item`, which is a layout shift.
 */
function drawnRange(view: EditorView) {
  return [view.viewport]
}

/**
 * Parses everything CodeMirror is about to draw, synchronously, and returns
 * that tree, so no line is drawn before its part of the tree exists. It is
 * bounded by the drawn range, so it stays fast; the rest keeps parsing in
 * the background.
 *
 * The returned tree must be the one used: `syntaxTree(state)` only picks up
 * the extra parsing on a later update, and decorating from it drew lines raw
 * for one frame and then reflowed them (a layout shift CI caught).
 */
function parseDrawn(view: EditorView): Tree | null {
  return ensureSyntaxTree(view.state, view.viewport.to, VISIBLE_PARSE_LIMIT_MS)
}

/** Effectively no limit: correctness first. A drawn range parses in ~1 ms. */
const VISIBLE_PARSE_LIMIT_MS = 10_000

/** Whether Enter is at the end of a typed table header, rather than in code or the hidden properties. */
function atTypedTableHeader(view: EditorView): boolean {
  const { state } = view
  const sel = state.selection.main
  if (!sel.empty) return false
  const line = state.doc.lineAt(sel.head)
  const text = line.text.trim()
  if (sel.head !== line.to || line.from < frontmatterEnd(state) || text.length < 3 || !/^\|.*\|$/.test(text)) return false
  for (let n: SyntaxNode | null = treeUpTo(state, line.from).resolveInner(line.from, 1); n; n = n.parent)
    if (/^(FencedCode|CodeBlock|InlineCode|CodeText)$/.test(n.name)) return false
  return true
}

/** Replays the normal Enter key after lazy table loading declines the typed header. */
function replayEnter(view: EditorView, original: KeyboardEvent): void {
  if (!view.dom.isConnected) return
  const replay = new KeyboardEvent('keydown', {
    key: original.key,
    code: original.code,
    bubbles: true,
    cancelable: true,
  }) as KeyboardEvent & { __tableEnterReplay?: boolean }
  replay.__tableEnterReplay = true
  view.contentDOM.dispatchEvent(replay)
}

/** Checks only Enter, ahead of the normal keymap, so a table header can load lazily without slowing ordinary typing. */
export const tableEnterHandler: Extension = Prec.highest(
  EditorView.domEventHandlers({
    keydown(event, view) {
      const key = event as KeyboardEvent & { __tableEnterReplay?: boolean }
      if (
        key.key !== 'Enter' ||
        key.isComposing ||
        key.shiftKey ||
        key.ctrlKey ||
        key.metaKey ||
        key.altKey ||
        key.__tableEnterReplay ||
        view.state.readOnly ||
        view.dom.dataset.mode === 'source' ||
        (key.target instanceof Element && key.target.closest('.tw')) ||
        !atTypedTableHeader(view)
      )
        return false
      key.preventDefault()
      key.stopPropagation()
      const replay = () => replayEnter(view, key)
      void import('../tables/lazy').then((m) => m.enterTable(view, replay)).catch(replay)
      return true
    },
  }),
)

/** Replays plain text when lazy spreadsheet parsing is unavailable. */
function pasteAsText(view: EditorView, text: string): void {
  if (view.dom.isConnected) view.dispatch(view.state.replaceSelection(text))
}

/** Detects table code on demand and removes its state field again when no visible table remains. */
class HiddenView {
  decorations: DecorationSet
  atomic: DecorationSet
  links: LinkSpan[]
  /** The link the caret is on (-1 for none), and the chips drawn for it. */
  on: number
  chips: DecorationSet
  private hasTable = false
  private tablesLoading = false
  private tablesActive = false
  /** Cell edits move link positions, but the already-drawn link marks can map until the next other editor update. */
  private linksStale = false

  constructor(view: EditorView) {
    const h = buildHidden(view.state, drawnRange(view), parseDrawn(view))
    this.decorations = h.decorations
    this.atomic = h.atomic
    this.links = h.links
    this.hasTable = h.hasTable
    this.on = focusedLink(this.links, view.state, view.hasFocus)
    this.chips = linkMarks(this.links, this.on, view.state.facet(linkState), view.state.doc)
    this.syncTables(view)
  }

  /** Maps existing decorations for Grid cell edits and rebuilds Markdown marks for other editor changes. */
  update(u: ViewUpdate) {
    const cellEdit = u.docChanged && u.transactions.length === 1 && u.transactions[0]!.isUserEvent('input.table.cell')
    if (cellEdit) {
      this.decorations = this.decorations.map(u.changes)
      this.atomic = this.atomic.map(u.changes)
      this.chips = this.chips.map(u.changes)
      this.linksStale = true
      return
    }

    const rebuilt = u.docChanged || u.viewportChanged || syntaxTree(u.state) !== syntaxTree(u.startState)
    if (rebuilt || this.linksStale) {
      const h = buildHidden(u.state, drawnRange(u.view), parseDrawn(u.view))
      this.decorations = h.decorations
      this.atomic = h.atomic
      this.links = h.links
      this.hasTable = h.hasTable
      this.linksStale = false
    }
    this.syncTables(u.view)
    // Moving the caret redraws the links only when it lands on, or leaves, one; the app's word
    // on a link's state redraws them too.
    const on = focusedLink(this.links, u.state, u.view.hasFocus)
    const asked = u.transactions.some((t) => t.effects.some((e) => e.is(refreshLinks)))
    if (rebuilt || asked || on !== this.on || u.startState.facet(linkState) !== u.state.facet(linkState)) {
      this.on = on
      this.chips = linkMarks(this.links, on, u.state.facet(linkState), u.state.doc)
    }
  }

  /** Starts or stops the table extension after the existing syntax pass has identified visible tables. */
  private syncTables(view: EditorView): void {
    if (this.hasTable) {
      if (this.tablesLoading || this.tablesActive) return
      this.tablesLoading = true
      void import('../tables/lazy')
        .then((m) => m.activate(view))
        .then(() => {
          this.tablesLoading = false
          this.tablesActive = this.hasTable
          if (!this.hasTable) void import('../tables/lazy').then((m) => m.deactivate(view)).catch(() => {})
        })
        .catch(() => {
          // A future view update can retry while the Markdown and its pending height remain visible.
          this.tablesLoading = false
        })
    } else if (this.tablesActive) {
      this.tablesActive = false
      void import('../tables/lazy').then((m) => m.deactivate(view)).catch(() => {})
    }
  }
}

/** Keeps syntax, links and table activation in the one view plugin already used for drawn lines. */
const hiddenPlugin = ViewPlugin.fromClass(HiddenView, {
  decorations: (v) => v.decorations,
  eventHandlers: {
    paste(event, view) {
      if (event.target instanceof Element && event.target.closest('.tw')) return false
      const data = event.clipboardData
      if (!data) return false
      const html = data.getData('text/html')
      const text = data.getData('text/plain')
      if (!/<table|\t.*\n/i.test(html + text)) return false
      event.preventDefault()
      event.stopPropagation()
      void import('../tables/lazy')
        .then((m) => m.pasteTable(view, html, text))
        .catch(() => pasteAsText(view, text))
      return true
    },
  },
  provide: (p) => [
    EditorView.atomicRanges.of((view) => view.plugin(p)?.atomic ?? Decoration.none),
    // Outermost, so each link is one chip around anything else drawn inside it.
    Prec.highest(EditorView.decorations.of((view) => view.plugin(p)?.chips ?? Decoration.none)),
  ],
})

/**
 * The drawn link whose words hold `pos` (ends included), or null. Only links
 * CodeMirror has drawn are known, which is every link the person can see or
 * reach with the caret.
 */
export function linkAt(view: EditorView, pos: number): LinkSpan | null {
  const links = view.plugin(hiddenPlugin)?.links ?? []
  return links[linkIndexAt(links, pos)] ?? null
}

/** Where the visible text of a line starts, after any hidden leading syntax. */
export function prefixEnd(state: EditorState, lineFrom: number): number {
  const line = state.doc.lineAt(lineFrom)
  const node = treeUpTo(state, line.to).resolveInner(line.from, 1)
  for (let n: typeof node | null = node; n; n = n.parent) if (n.name === 'FencedCode') return line.from
  let pos = line.from
  const head = /^(> ?)*(#{1,6} )?/.exec(line.text)
  if (head?.[0]) pos = line.from + head[0].length
  const rest = state.doc.sliceString(pos, line.to)
  const li = TASK.exec(rest)
  if (li && (li[4] || /[-*+]/.test(li[2]!))) pos += li[0].length
  return pos
}

/**
 * Keeps the caret where typing makes sense, and the hidden properties (the frontmatter) out of reach.
 *
 * Hiding the properties with a decoration is not enough: the block's last line break sits one
 * character before the body, so a Backspace at the start of the body deletes it and the closing
 * `---` joins the first body line. The block no longer closes and the whole of it turns into
 * ordinary text (the production bug of 11-06). Atomic ranges only steer the caret; they never
 * stop an edit. So, for everything but the shared document's own changes and the properties
 * writer (user event `input.properties`: Style, tags, the cover, the one way in; typing never has it):
 *
 * - a change that starts inside the properties refuses the whole transaction (Backspace at the
 *   body's start, a letter typed at position 0, a composition);
 * - no selection end is inside them, so select all + delete and paste over everything replace
 *   the body and nothing else, and copy and cut never carry a property;
 * - a lone caret is kept out of hidden leading syntax (after Home, a click at the start of a
 *   heading, arrowing into a list item), so typing always lands in the visible text and never
 *   inside `# ` or `- [ ] `.
 *
 * Source mode does not install this, so every character stays editable there.
 */
const keepCursorOutOfPrefix = EditorState.transactionFilter.of((tr) => {
  if (tr.annotation(ySyncAnnotation) != null) return tr
  const start = frontmatterEnd(tr.startState)
  if (start && !tr.isUserEvent('input.properties')) {
    let hit = false
    tr.changes.iterChangedRanges((from) => void (hit ||= from < start))
    if (hit) return []
  }
  if (!tr.selection) return tr
  const state = tr.state
  const body = frontmatterEnd(state)
  const was = tr.selection
  let sel = was
  if (was.ranges.some((r) => r.anchor < body || r.head < body))
    sel = EditorSelection.create(was.ranges.map((r) => EditorSelection.range(Math.max(body, r.anchor), Math.max(body, r.head))), was.mainIndex)
  if (sel.ranges.length === 1 && sel.main.empty) {
    const end = prefixEnd(state, state.doc.lineAt(sel.main.head).from)
    if (sel.main.head < end) sel = EditorSelection.single(end)
  }
  return sel === was ? tr : [tr, { selection: sel, sequential: true }]
})

/** Hidden markdown syntax, the default editing mode. */
export const hiddenSyntax = [hiddenFrontmatter, hiddenPlugin, keepCursorOutOfPrefix]
