/**
 * Blocks: the pieces of a note that controls attach to. A block is a
 * paragraph, one list item, a heading, a quote, a code block or a rule.
 *
 * The note's title (its H1), blank lines and the hidden frontmatter are
 * never blocks, so they never get a handle or buttons.
 */

import type { EditorState } from '@codemirror/state'
import type { SyntaxNode } from '@lezer/common'
import { frontmatterEnd } from '../syntax/frontmatter'
import { treeUpTo } from '../syntax/hidden'

export interface Block {
  from: number
  to: number
  /** The syntax node name, e.g. `Paragraph`, `ListItem`, `ATXHeading2`. */
  kind: string
}

const NOT_BLOCKS = new Set(['ATXHeading1', 'SetextHeading1'])

/** The block on the line containing `pos`, or null if that line has none. */
export function blockAt(state: EditorState, pos: number): Block | null {
  const line = state.doc.lineAt(pos)
  if (line.text.trim() === '') return null
  if (line.from < frontmatterEnd(state)) return null
  // Look from the line's first visible character: indentation belongs to
  // the parent list item, the text to the nested one.
  const indent = line.text.length - line.text.trimStart().length
  let node: SyntaxNode | null = treeUpTo(state, line.to).resolveInner(line.from + indent, 1)
  let top: SyntaxNode | null = null
  let item: SyntaxNode | null = null
  for (; node && node.parent; node = node.parent) {
    if (node.name === 'ListItem' && !item) item = node
    if (!node.parent.parent) top = node
  }
  if (!top) return null
  if (NOT_BLOCKS.has(top.name)) return null
  const chosen = item ?? top
  const end = item ? itemOwnEnd(state, item) : chosen.to
  return { from: chosen.from, to: end, kind: chosen.name }
}

/** A list item's own lines, without its nested child list. */
function itemOwnEnd(state: EditorState, item: SyntaxNode): number {
  for (let c = item.firstChild; c; c = c.nextSibling) {
    if (c.name === 'BulletList' || c.name === 'OrderedList') {
      return state.doc.lineAt(c.from).from - 1
    }
  }
  return item.to
}

/** Every block with a line between `from` and `to`, top to bottom. */
export function blocksIn(state: EditorState, from: number, to: number): Block[] {
  const out: Block[] = []
  const doc = state.doc
  for (let n = doc.lineAt(from).number, end = doc.lineAt(to).number; n <= end; ) {
    const b = blockAt(state, doc.line(n).from)
    if (b && !out.some((o) => o.from === b.from)) out.push(b)
    n = Math.max(n, b ? doc.lineAt(b.to).number : n) + 1
  }
  return out
}

/** The next block below (1) or above (-1) the one at `pos`, or null at the end of the note. */
export function blockBeside(state: EditorState, pos: number, dir: 1 | -1): Block | null {
  const doc = state.doc
  const here = blockAt(state, pos)
  let n = doc.lineAt(dir > 0 ? (here?.to ?? pos) : (here?.from ?? pos)).number + dir
  for (; n >= 1 && n <= doc.lines; n += dir) {
    const b = blockAt(state, doc.line(n).from)
    if (b && b.from !== here?.from) return b
  }
  return null
}
