/**
 * The note's headings, read from the syntax tree: where each starts, how deep it is and what it says.
 * The page's top pills follow them while you read (#285, #469). The editor only draws the lines on
 * screen, so this is the one place that knows all of them.
 */

import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import type { EditorState } from '@codemirror/state'
import { visibleText } from './visible-text'
import { frontmatterEnd } from './frontmatter'

export interface Heading {
  /** Where the heading's line starts. */
  from: number
  /** 1 to 4. */
  level: number
  /** The words, without the `#` marks. */
  text: string
}

/**
 * The headings. `whole` (the default) asks the parser for the whole note first (50 ms at most): the Outline lists
 * every heading. `whole: false` reads only as far as the note is parsed, without parsing more: the page's top
 * pills need only the headings above what is drawn, and the tree always reaches the drawn lines (plan 1c, so
 * opening a long note never parses all of it just for the pills). Read again when `parsedTo` moves.
 */
export function headingsOf(state: EditorState, whole = true): Heading[] {
  const out: Heading[] = []
  const body = frontmatterEnd(state)
  const tree = whole ? (ensureSyntaxTree(state, state.doc.length, 50) ?? syntaxTree(state)) : syntaxTree(state)
  tree.iterate({
    enter(n) {
      const m = /^ATXHeading([1-4])$/.exec(n.name)
      if (!m) return
      if (n.from < body) return false
      const text = state.doc
        .lineAt(n.from)
        .text.replace(/^\s{0,3}#{1,6}\s*/, '')
        .replace(/\s+#+\s*$/, '')
        .trim()
      const title = visibleText(text)
      if (title) out.push({ from: n.from, level: Number(m[1]), text: title })
      return false
    },
  })
  return out
}

/** How far the note's syntax tree reaches: it grows as the rest of a long note is parsed at idle. */
export const parsedTo = (state: EditorState): number => syntaxTree(state).length
