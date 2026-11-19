/**
 * The syntax tree a state-built part (embeds, tables, code colours) draws from, plan 1c.
 *
 * Those parts draw only as far as the tree reaches and never ask for the whole note (opening a long note would parse
 * all of it). But after an edit, the language field re-parses with a small time budget, and on a busy machine it can
 * stop short of where the tree reached before. A part drawing from that shorter tree would take down what it drew
 * past the cut (a table's grid while you type in it) and draw it again a moment later. So after an edit a part asks
 * for the tree to reach at least as far as it did before (`upto`, mapped through the edit), with the same small
 * budget the table finder always had. That is never more than was already parsed, so the first paint stays cheap.
 */

import { ensureSyntaxTree, syntaxTree } from '@codemirror/language'
import type { EditorState } from '@codemirror/state'
import type { Tree } from '@lezer/common'

/** The most an edit waits for the parser to catch up with where the tree reached before. */
const EDIT_MS = 25

/** The state's tree, parsed on to `upto` if it stops short of it (within `EDIT_MS`; else the tree there is). */
export function treeTo(state: EditorState, upto: number): Tree {
  const tree = syntaxTree(state)
  if (tree.length >= Math.min(upto, state.doc.length)) return tree
  return ensureSyntaxTree(state, Math.min(upto, state.doc.length), EDIT_MS) ?? tree
}
