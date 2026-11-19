/**
 * Embeds (a picture, a file, a video, a recording, a gallery, a note card) drawn in place of their markdown, as one editor extension the app adds only when it is needed.
 *
 * `![alt](<../Photos/2026/2026-10-17/shell.jpg>)` alone on a line stays in the file. With this extension
 * in the editor (it stands down in source mode), an embed takes its place as one atomic piece: the caret steps over
 * it and Backspace takes it out whole. The editor knows nothing about where files live: the app's
 * {@link EmbedWidgetFor} answers for the text of a line, with a widget or null (null leaves the markdown as text).
 *
 * Why a field of its own, and not part of `hidden.ts`: a note without embeds must run exactly the code it ran
 * before embeds existed, and a note with embeds must not pay for them on every key. The field maps its
 * decorations through each edit and looks again only at the lines the edit touched, so typing in another
 * block costs one line check per changed range, however many embeds the note has. Everything here sits in
 * the app's lazy attachments chunk (a subpath export): the first paint never reaches it.
 *
 * Viewport first (plan 1c): whether a line is in code comes from the syntax tree the editor already has,
 * never from parsing further. Opening a long note parses only what is drawn (see `parse-ahead.ts`), so the
 * lines past the parsed part wait, and are looked at as the tree grows (at idle, or before a scroll draws
 * them). Only how the note is drawn waits; the text is the note's own, whole, from the first frame.
 */

import { syntaxTree } from '@codemirror/language'
import type { SyntaxNode, Tree } from '@lezer/common'
import { treeTo } from './tree-to'
import { type EditorState, type Extension, type Range, StateEffect, StateField } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, type WidgetType } from '@codemirror/view'

/** The widget that draws a line (given its text) as an embed, or null to leave the markdown as text. */
export type EmbedWidgetFor = (line: string) => WidgetType | null

/** Dispatch this when the app learned something about a file (its size): embeds are drawn again. */
export const refreshEmbeds = StateEffect.define<null>()

/** What is drawn: everything (line classes and embeds), and the embeds alone, which the caret steps over. */
interface Drawn {
  all: DecorationSet
  atomic: DecorationSet
  /** Where the last drawn embed ends (-1 when there is none): an edit after it moves none of them. */
  end: number
  /** The lines up to here have been looked at; the ones after wait for the syntax tree to reach them. */
  looked: number
}

/** Where the last decoration of `set` ends, or -1. Read only when the sets are made again, never for a plain keystroke. */
function endOf(set: DecorationSet): number {
  let e = -1
  for (const c = set.iter(); c.value; c.next()) e = Math.max(e, c.to)
  return e
}

const made = (all: DecorationSet, atomic: DecorationSet, looked = 0): Drawn => ({ all, atomic, end: endOf(atomic), looked })

const lineClass = Decoration.line({ class: 'cm-embed-line' })

/** Block kinds where `![…](<…>)` is code or markup, never an embed. */
const NOT_MARKDOWN = new Set(['FencedCode', 'CodeBlock', 'HTMLBlock', 'CommentBlock'])

/** Whether `pos` sits in code, where the text stays text, by the parsed `tree`. */
function inCode(tree: Tree, pos: number): boolean {
  for (let n: SyntaxNode | null = tree.resolveInner(pos, 1); n; n = n.parent) if (NOT_MARKDOWN.has(n.name)) return true
  return false
}

/**
 * The decorations for the lines from the one holding `from` to the one holding `to`. Only a line that starts with `![` is
 * looked at, and only one the parsed tree covers whole (`line.to <= tree.length`): what comes before a line decides whether
 * it is code, so a covered line's answer never changes as the tree grows.
 */
function look(state: EditorState, widgetFor: EmbedWidgetFor, from: number, to: number, tree: Tree = syntaxTree(state)) {
  const all: Range<Decoration>[] = []
  const atomic: Range<Decoration>[] = []
  const doc = state.doc
  for (let line = doc.lineAt(from); ; line = doc.line(line.number + 1)) {
    if (line.to > tree.length) break
    if (line.text.startsWith('![')) {
      const widget = widgetFor(line.text)
      if (widget && !inCode(tree, line.from)) {
        const embed = Decoration.replace({ widget }).range(line.from, line.to)
        all.push(lineClass.range(line.from), embed)
        atomic.push(embed)
      }
    }
    if (line.to >= to || line.number === doc.lines) break
  }
  return { all, atomic }
}

/** How far the parsed tree covers whole lines: the lines up to here have been looked at. */
const lookedTo = (state: EditorState, tree: Tree = syntaxTree(state)): number => {
  if (tree.length >= state.doc.length) return state.doc.length
  // The line the tree ends in is not covered whole yet: it is looked at with the next part.
  return Math.max(0, state.doc.lineAt(tree.length).from - 1)
}

/** Draws the lines from `from` to `to` again (both ends whole lines). */
function redo(value: Drawn, state: EditorState, widgetFor: EmbedWidgetFor, from: number, to: number, tree: Tree = syntaxTree(state)): Drawn {
  const d = look(state, widgetFor, from, to, tree)
  let had = false
  value.all.between(from, to, () => ((had = true), false))
  if (!had && !d.all.length) return { ...value, looked: lookedTo(state, tree) }
  const all = value.all.update({ filter: () => false, filterFrom: from, filterTo: to, add: d.all, sort: true })
  const atomic = value.atomic.update({ filter: () => false, filterFrom: from, filterTo: to, add: d.atomic, sort: true })
  return { ...made(all, atomic), looked: lookedTo(state, tree) }
}

/**
 * The extension: draws the lines `widgetFor` answers for. Adding it to a note that already holds embeds draws
 * them at once; it follows edits from then on.
 */
export function embedLines(widgetFor: EmbedWidgetFor): Extension {
  const field = StateField.define<Drawn>({
    create(state) {
      const d = look(state, widgetFor, 0, state.doc.length)
      return made(Decoration.set(d.all, true), Decoration.set(d.atomic, true), lookedTo(state))
    },
    update(value, tr) {
      const state = tr.state
      if (tr.effects.some((e) => e.is(refreshEmbeds))) {
        const d = look(state, widgetFor, 0, state.doc.length)
        return made(Decoration.set(d.all, true), Decoration.set(d.atomic, true), lookedTo(state))
      }
      if (!tr.docChanged) {
        // The tree grew (idle parsing, or a scroll that draws further): look at the lines it reached, and only those.
        const to = lookedTo(state)
        if (to <= value.looked) return value
        return redo(value, state, widgetFor, state.doc.lineAt(value.looked).from, to)
      }
      let { all, atomic } = value
      // Text typed after the last embed moves none of them: nothing to map (the usual keystroke in a long note).
      let before = false
      tr.changes.iterChangedRanges((fromA) => void (before ||= fromA <= value.end))
      let redrawn = false
      if (before) {
        all = all.map(tr.changes)
        atomic = atomic.map(tr.changes)
        redrawn = true
      }
      // The tree reaches at least as far as before the edit (see `treeTo`), so nothing drawn is taken down by a short re-parse.
      const mapped = tr.changes.mapPos(value.looked, -1)
      const tree = treeTo(state, mapped)
      // The lines looked at so far, after this edit: never past what the tree covers whole.
      const looked = Math.min(mapped, lookedTo(state, tree))
      tr.changes.iterChangedRanges((_fa, _ta, from, to) => {
        const a = state.doc.lineAt(from).from
        const b = state.doc.lineAt(to).to
        // An edit past the parsed part is looked at once the tree reaches it.
        if (a > looked) return
        const d = look(state, widgetFor, a, b, tree)
        // Nothing drawn there before and nothing now (the usual keystroke): leave the sets as they are.
        let had = false
        all.between(a, b, () => ((had = true), false))
        if (!had && !d.all.length) return
        all = all.update({ filter: () => false, filterFrom: a, filterTo: b, add: d.all, sort: true })
        atomic = atomic.update({ filter: () => false, filterFrom: a, filterTo: b, add: d.atomic, sort: true })
        redrawn = true
      })
      // The lines between the old end and the new tree's end were never looked at: look now.
      const to = lookedTo(state, tree)
      if (to > looked) return redo(made(all, atomic, looked), state, widgetFor, state.doc.lineAt(looked).from, to, tree)
      if (looked !== value.looked) return made(all, atomic, looked)
      return redrawn ? made(all, atomic, looked) : value
    },
    // Source mode shows the file as it is: the editor says so in `data-mode` before it redraws (see mode.ts).
    provide: (f) => [
      EditorView.decorations.of((view) => (view.dom.dataset.mode === 'source' ? Decoration.none : view.state.field(f).all)),
      EditorView.atomicRanges.of((view) => (view.dom.dataset.mode === 'source' ? Decoration.none : view.state.field(f).atomic)),
    ],
  })
  return field
}
