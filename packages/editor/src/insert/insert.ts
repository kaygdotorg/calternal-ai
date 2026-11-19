/**
 * Putting a block action into the note, from either place (the `/` menu or
 * the Insert sheet). A block action writes its markdown into the empty line
 * the caret is on, or on a new line after the line it is on. A picker action
 * types its trigger and lets the inline picker take over. Nothing here
 * animates: it is a keyboard-sized action.
 */

import { EditorSelection } from '@codemirror/state'
import type { EditorView } from '@codemirror/view'
import { frontmatterEnd } from '../syntax/frontmatter'
import { askPicker } from '../transforms/picker'
import type { BlockAction } from './registry'

/** What a text-only action writes. */
const markdownOf = (a: BlockAction) => (typeof a.markdown === 'function' ? a.markdown() : a.markdown)

/** Writes the action's markdown at the caret's line. */
export function insertBlockAction(view: EditorView, a: BlockAction): void {
  // A block that does something else than write markdown (a file chooser) does it, and writes nothing.
  if (a.run) return a.run(view)
  if (a.pick) return startPick(view, a)
  const { state } = view
  const line = state.doc.lineAt(state.selection.main.head)
  const md = markdownOf(a)
  const caret = a.caret ?? md.length
  // A `---` straight under a line of text would make that text a heading: a blank line goes in first (a divider is `---`).
  const rule = md.startsWith('---')
  if (!line.text.trim() && line.from >= frontmatterEnd(state)) {
    const gap = rule && line.number > 1 && state.doc.line(line.number - 1).text.trim() ? '\n' : ''
    view.dispatch({ changes: { from: line.from, to: line.to, insert: gap + md }, selection: EditorSelection.cursor(line.from + gap.length + caret), userEvent: 'input.complete', scrollIntoView: true })
    return
  }
  const lead = rule ? '\n\n' : '\n'
  view.dispatch({ changes: { from: line.to, insert: lead + md }, selection: EditorSelection.cursor(line.to + lead.length + caret), userEvent: 'input.complete', scrollIntoView: true })
}

/**
 * A picker action: the trigger is typed at the caret (a space first when a word sits right
 * before it), which opens the same picker typing it would. The kind is passed along so `@`
 * can mean "a day" and `#` can open with nothing after it.
 */
export function startPick(view: EditorView, a: BlockAction): void {
  if (!a.pick) return
  const head = view.state.selection.main.head
  const before = view.state.doc.sliceString(Math.max(0, head - 1), head)
  const lead = before && !/\s|\(/.test(before) ? ' ' : ''
  view.dispatch({
    changes: { from: head, insert: lead + a.pick.trigger },
    selection: EditorSelection.cursor(head + lead.length + a.pick.trigger.length),
    effects: askPicker.of(a.pick.kind),
    userEvent: 'input.complete',
  })
}
