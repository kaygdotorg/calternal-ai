/**
 * Everything a code block adds to the editor, all of it a lazy chunk the app fetches only for a note that has
 * a fenced block (apps/web/src/lib/lazy-code.ts): the language button in the opening fence's place, the
 * fence lines as the block's 11 px of padding (they are never drawn), and syntax colours (`tk-*` classes
 * in the mockup's palette). Each language's parser is a further chunk, fetched only when a block names that
 * language. A fence naming no known language is plain text and gets no colours.
 *
 * Cost on the typing path: none for text outside code. A change inside a code block parses that one block
 * again and repaints it; a change that adds or removes a fence, or a parser arriving, repaints every code
 * block once (they are found by walking the block containers, never the words). Blocks over 40 000
 * characters stay plain, so one huge paste can't make a keystroke slow.
 *
 * Viewport first (plan 1c): blocks are painted as far as the syntax tree reaches. Opening a long note parses
 * only what is drawn (`syntax/parse-ahead.ts`); as the tree grows (at idle, or before a scroll draws further)
 * only the blocks it newly reached are painted, never every block again. The text is never touched.
 */

import { syntaxTree } from '@codemirror/language'
import { type EditorState, type Range, StateEffect, StateField, type Transaction } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import { CodeLangWidget } from './lang-widget'
import type { Parser, Tree } from '@lezer/common'
import { type Tag, getStyleTags, tags as t } from '@lezer/highlight'
import { type Lang, fenceWord, langOf } from './langs'
import { isLezer, loadParser } from './grammars/lezer'
import { spans } from './grammars/words'
import { treeTo } from '../syntax/tree-to'

const BIGGEST = 40_000

/**
 * Which `tk-*` class a lezer node gets, read off the styling its parser declared (`styleTags`). Not
 * `highlightTree`/`tagHighlighter`: they would put 1.5 KB of the highlighter into the first-paint bundle,
 * and this needs none of their inheritance rules, since a coloured node's children keep its colour.
 */
const CLASSES = new Map<Tag, string>([
  ...[t.keyword, t.bool, t.null, t.self, t.modifier, t.operatorKeyword].map((x) => [x, 'tk-kw'] as const),
  ...[t.string, t.special(t.string), t.regexp].map((x) => [x, 'tk-str'] as const),
  [t.number, 'tk-num'],
  [t.comment, 'tk-com'],
  ...[t.function(t.variableName), t.function(t.propertyName), t.macroName].map((x) => [x, 'tk-fn'] as const),
])

const marks = new Map<string, Decoration>()
const mark = (cls: string) => marks.get(cls) ?? (marks.set(cls, Decoration.mark({ class: cls })), marks.get(cls)!)

/** The parsers that have arrived, and the ones asked for. */
const parsers = new Map<Lang, Parser>()
const asked = new Set<Lang>()
const arrived = StateEffect.define<Lang>()

const CONTAINERS = new Set(['Blockquote', 'BulletList', 'OrderedList', 'ListItem'])

interface Code {
  lang: Lang
  /** The opening fence line, its word, and the closing fence line (or -1 when the block is not closed). */
  open: number
  word: string
  close: number
  /** The text between the fences, and where it starts. */
  from: number
  to: number
  /** The whole block, for finding it again. */
  start: number
  end: number
}

/** Every fenced code block between `from` and `to`, inside quotes and lists too. */
function blocks(state: EditorState, from: number, to: number, tree: Tree = syntaxTree(state)): Code[] {
  const out: Code[] = []
  tree.iterate({
    from,
    to,
    enter(n) {
      if (n.name === 'FencedCode') {
        const first = state.doc.lineAt(n.from)
        const last = state.doc.lineAt(n.to)
        const closed = last.number !== first.number && /^\s*(```|~~~)/.test(last.text)
        const text = n.node.getChild('CodeText')
        const word = fenceWord(first.text)
        out.push({ lang: langOf(word), open: first.from, word, close: closed ? last.from : -1, from: text?.from ?? first.to, to: text?.to ?? first.to, start: first.from, end: last.to })
        return false
      }
      return n.name === 'Document' || CONTAINERS.has(n.name)
    },
  })
  return out
}

const fenceLine = Decoration.line({ class: 'cm-codeblock-first cm-codeblock-fence' })
const closeLine = Decoration.line({ class: 'cm-codeblock-fence' })

/** The block's fence decorations: the button, and the two fence lines as padding. */
function fences(code: Code, out: Range<Decoration>[]) {
  out.push(fenceLine.range(code.open), Decoration.widget({ widget: new CodeLangWidget(code.word), side: -1 }).range(code.open))
  if (code.close >= 0) out.push(closeLine.range(code.close))
}

function paint(state: EditorState, code: Code, out: Range<Decoration>[]) {
  if (code.lang === 'plain' || code.to <= code.from || code.to - code.from > BIGGEST) return
  const text = state.doc.sliceString(code.from, code.to)
  if (code.lang === 'shell' || code.lang === 'sql') {
    for (const s of spans(text, code.lang)) out.push(mark(s.cls).range(code.from + s.from, code.from + s.to))
    return
  }
  const parser = parsers.get(code.lang)
  if (!parser) return void asked.add(code.lang)
  const tree: Tree = parser.parse(text)
  tree.iterate({
    enter(n) {
      const rule = getStyleTags(n)
      if (!rule) return
      for (const tag of rule.tags)
        for (const x of tag.set) {
          const cls = CLASSES.get(x)
          if (cls) {
            out.push(mark(cls).range(code.from + n.from, code.from + n.to))
            // A coloured node keeps one colour all through, so its children are not looked at.
            return false
          }
        }
      return
    },
  })
}

interface Painted {
  marks: DecorationSet
  fence: DecorationSet
  /** How far the syntax tree reached when these were painted: blocks past it wait for the tree. */
  upTo: number
}

function everything(state: EditorState, tree: Tree = syntaxTree(state)): Painted {
  const marks: Range<Decoration>[] = []
  const fence: Range<Decoration>[] = []
  for (const code of blocks(state, 0, state.doc.length, tree)) {
    fences(code, fence)
    paint(state, code, marks)
  }
  return { marks: marks.length ? Decoration.set(marks, true) : Decoration.none, fence: fence.length ? Decoration.set(fence, true) : Decoration.none, upTo: tree.length }
}

/**
 * The tree grew past `value.upTo`: paints the blocks it newly reached, including the one it used to end in (that one
 * was cut short where the old tree stopped, so it is painted again whole).
 */
function grow(value: Painted, state: EditorState): Painted {
  const end = syntaxTree(state).length
  if (end <= value.upTo) return value
  const found = blocks(state, Math.max(0, value.upTo - 1), end)
  if (!found.length) return { ...value, upTo: end }
  const from = Math.min(value.upTo, found[0]!.start)
  const m: Range<Decoration>[] = []
  const f: Range<Decoration>[] = []
  for (const code of found) {
    fences(code, f)
    paint(state, code, m)
  }
  const marks = value.marks.update({ filterFrom: from, filterTo: end, filter: () => false, add: m, sort: true })
  const fence = value.fence.update({ filterFrom: from, filterTo: end, filter: () => false, add: f, sort: true })
  return { marks, fence, upTo: end }
}

/** Whether a change added or removed a fence line (a backtick or tilde, new or old). */
function fenceChanged(tr: Transaction): boolean {
  let hit = false
  tr.changes.iterChangedRanges((fromA, toA, fromB, toB) => {
    if (hit) return
    hit = /[`~]/.test(tr.startState.doc.sliceString(fromA, toA)) || /[`~]/.test(tr.state.doc.sliceString(fromB, toB))
  })
  return hit
}

const field = StateField.define<Painted>({
  create: everything,
  update(value, tr) {
    const tree = syntaxTree(tr.state)
    if (tr.effects.some((e) => e.is(arrived))) return everything(tr.state)
    if (!tr.docChanged) {
      if (tree === syntaxTree(tr.startState)) return value
      // The usual case: the parser went further (at idle, or for lines about to be drawn).
      return tree.length > value.upTo ? grow(value, tr.state) : everything(tr.state)
    }
    if (fenceChanged(tr)) return everything(tr.state, treeTo(tr.state, tr.changes.mapPos(value.upTo, -1)))
    let { marks, fence } = value
    // The tree reaches at least as far as before the edit (see `treeTo`), so no block is repainted from a cut-short tree.
    const mapped = tr.changes.mapPos(value.upTo, -1)
    const edited = treeTo(tr.state, mapped)
    const upTo = Math.min(mapped, edited.length)
    marks = marks.map(tr.changes)
    fence = fence.map(tr.changes)
    tr.changes.iterChangedRanges((_a, _b, from, to) => {
      for (const code of blocks(tr.state, from, to, edited)) {
        const m: Range<Decoration>[] = []
        paint(tr.state, code, m)
        marks = marks.update({ filterFrom: code.from, filterTo: code.to, filter: () => false, add: m, sort: true })
        // The fence lines too: a change on the opening line is a new word for the button.
        const f: Range<Decoration>[] = []
        fences(code, f)
        fence = fence.update({ filterFrom: code.start, filterTo: code.end, filter: () => false, add: f, sort: true })
      }
    })
    return grow({ marks, fence, upTo }, tr.state)
  },
  provide: (f) => [
    EditorView.decorations.from(f, (v) => v.marks),
    // The fence lines and the button are for the reading view; source mode shows the file as it is.
    EditorView.decorations.of((view) => (view.dom.dataset.mode === 'source' ? Decoration.none : view.state.field(f).fence)),
  ],
})

/** Fetches the parsers the note's blocks asked for, then paints again. */
const fetcher = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {
      this.fetch()
    }
    update(_u: ViewUpdate) {
      if (asked.size) this.fetch()
    }
    fetch() {
      for (const lang of [...asked]) {
        asked.delete(lang)
        if (!isLezer(lang) || parsers.has(lang)) continue
        void loadParser(lang).then(
          (p) => {
            parsers.set(lang, p)
            this.view.dispatch({ effects: arrived.of(lang) })
          },
          () => {},
        )
      }
    }
  },
)

/** The fence lines, the language button and the colours: what a code block adds to the editor. */
export const codeBlockParts = [field, fetcher]
