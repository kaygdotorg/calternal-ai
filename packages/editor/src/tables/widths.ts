/**
 * Column widths a person dragged. They are the only thing a table keeps outside its own pipes, and they live in the
 * note's frontmatter, as the mockup and ADR 0056 / 0068 have them: character counts, one list per table in document
 * order, `null` for a column that sizes itself:
 *
 *   table-widths: [[14, null, 4, 5]]
 *
 * The table's text is never touched.
 */

import type { EditorState } from '@codemirror/state'
import type { EditorView } from '@codemirror/view'
import { frontmatterEnd } from '../syntax/frontmatter'

export type Widths = (number | null)[]

const LINE = /^table-widths:[ \t]*(.*)$/m

const clean = (e: unknown): Widths | null =>
  Array.isArray(e) ? e.map((x) => (typeof x === 'number' && x > 0 && x < 1000 ? Math.round(x) : null)) : null

/** The lists in the frontmatter, by table (an entry is null when that table has none). */
export function readWidths(state: EditorState): (Widths | null)[] {
  const end = frontmatterEnd(state)
  const m = end ? LINE.exec(state.sliceDoc(0, end)) : null
  if (!m) return []
  try {
    const all: unknown = JSON.parse(m[1]!)
    return Array.isArray(all) ? all.map(clean) : []
  } catch {
    return []
  }
}

/** The new `table-widths` value for the lists, or null when no table has a width of its own. */
export function widthsText(all: (Widths | null)[]): string | null {
  const lists = all.map((w) => (w?.some(Boolean) ? w : null))
  while (lists.length && lists[lists.length - 1] === null) lists.pop()
  return lists.length ? JSON.stringify(lists).replace(/,/g, ', ') : null
}

/** Sets (or, with null, forgets) one table's widths. `count` is how many tables the note has. */
export function writeWidths(view: EditorView, index: number, count: number, w: Widths | null): void {
  const { state } = view
  const all = readWidths(state)
  while (all.length < count) all.push(null)
  all[index] = w
  const text = widthsText(all)
  const end = frontmatterEnd(state)
  // The editor refuses every edit of the frontmatter except one tagged `input.properties` (hidden.ts), so the widths
  // line is written with that tag; any other tag is dropped without a sound and the widths would not survive a reload.
  const user = 'input.properties'
  if (!end) {
    if (text) view.dispatch({ changes: { from: 0, insert: `---\ntable-widths: ${text}\n---\n` }, userEvent: user })
    return
  }
  const head = state.sliceDoc(0, end)
  const m = LINE.exec(head)
  if (m) {
    const from = m.index
    const to = from + m[0].length
    const rest = head.slice(0, from) + head.slice(Math.min(to + 1, head.length))
    if (!text && rest.replace(/^---\r?\n/, '').replace(/---[ \t]*\r?\n?$/, '').trim() === '')
      // Nothing else was in the frontmatter: the whole block goes, so no empty `---` pair is left behind.
      view.dispatch({ changes: { from: 0, to: end }, userEvent: user })
    else if (!text) view.dispatch({ changes: { from, to: Math.min(to + 1, end) }, userEvent: user })
    else view.dispatch({ changes: { from, to, insert: `table-widths: ${text}` }, userEvent: user })
  } else if (text) {
    // Before the closing `---`.
    const close = state.doc.lineAt(end - 1)
    view.dispatch({ changes: { from: close.from, insert: `table-widths: ${text}\n` }, userEvent: user })
  }
}
