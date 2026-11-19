/**
 * Keys that mean something before Vim sees them (#538):
 *
 * - Esc in normal mode does nothing more than Vim's own (it drops a half-typed command and the
 *   search highlight): it never selects the block or leaves the note, as Esc does without Vim.
 * - Esc while the `[[`, `@` or `#` picker shows closes the picker and stays in insert mode.
 * - With blocks selected (`V`), outside insert mode, the keys act on blocks: j and k (or ↓ and ↑)
 *   take in the next block, a count first takes in that many; d, x or Delete delete them; y copies
 *   them for `p`; > and < nest list items in and out; Esc or V puts them down, the caret at the
 *   block it ended on. Other letters do nothing, so a stray key never edits blocks as words.
 */

import { EditorSelection, Prec } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { Vim } from '@replit/codemirror-vim'
import { type Block, blockAt, blockBeside, blocksIn } from '../blocks/blocks'
import { shownBlocks } from '../blocks/selected'
import { closePicker } from '../transforms/picker'
import { type CM, getCM } from './commands'
import type { VimHost } from './host'

/** The block selection's two ends: where it began, and the block that moves. */
function ends(view: EditorView): { anchor: Block; head: Block } | null {
  const { anchor, head } = view.state.selection.main
  const a = blockAt(view.state, head >= anchor ? anchor : anchor - 1)
  const h = blockAt(view.state, head >= anchor ? Math.max(anchor, head - 1) : head)
  return a && h ? { anchor: a, head: h } : null
}

function walk(view: EditorView, host: VimHost, dir: 1 | -1, count: number): void {
  const e = ends(view)
  if (!e) return
  let head = e.head
  for (let i = 0; i < count; i++) head = blockBeside(view.state, head.from, dir) ?? head
  const from = Math.min(e.anchor.from, head.from)
  const to = Math.max(e.anchor.to, head.to)
  host.selectBlocks(view, blocksIn(view.state, from, to), head.from < e.anchor.from ? 'start' : 'end')
}

function text(view: EditorView): string {
  const { from, to } = view.state.selection.main
  return view.state.doc.sliceString(view.state.doc.lineAt(from).from, to)
}

function putDown(view: EditorView): void {
  const e = ends(view)
  view.dispatch({ selection: EditorSelection.cursor(e?.head.from ?? view.state.selection.main.head) })
}

function blockKeys(view: EditorView, host: VimHost, e: KeyboardEvent, count: { n: string }): boolean {
  if (!shownBlocks(view.state) || e.ctrlKey || e.metaKey || e.altKey) return false
  const n = Math.max(1, Number(count.n) || 1)
  const key = e.key
  if (/^[1-9]$/.test(key) || (key === '0' && count.n)) {
    count.n += key
    return true
  }
  count.n = ''
  const yank = () => Vim.getRegisterController().pushText('"', 'yank', text(view), true)
  if (key === 'j' || key === 'ArrowDown') walk(view, host, 1, n)
  else if (key === 'k' || key === 'ArrowUp') walk(view, host, -1, n)
  else if (key === 'Escape' || key === 'V') putDown(view)
  else if (key === 'y') {
    yank()
    putDown(view)
  } else if (key === 'd' || key === 'x' || key === 'Delete' || key === 'Backspace') {
    yank()
    host.deleteBlocks(view)
  } else if (key === '>' || key === '<') {
    const { from, to } = view.state.selection.main
    const [first, last] = [view.state.doc.lineAt(from).number, view.state.doc.lineAt(to).number]
    host.nest(view, key === '>' ? 1 : -1)
    const doc = view.state.doc
    host.selectBlocks(view, blocksIn(view.state, doc.line(first).from, doc.line(last).to), 'end')
  } else if (key.length !== 1 && key !== 'Enter') return false
  return true
}

export function vimKeys(host: VimHost) {
  const count = { n: '' }
  return Prec.highest(
    EditorView.domEventHandlers({
      keydown(e, view) {
        if (e.isComposing || e.keyCode === 229) return false
        const cm = getCM(view) as unknown as CM | null
        const vim = cm?.state.vim
        if (!vim) return false
        if (!vim.insertMode && blockKeys(view, host, e, count)) {
          e.preventDefault()
          return true
        }
        if (e.key !== 'Escape') return false
        if (closePicker(view)) {
          e.preventDefault()
          return true
        }
        if (vim.insertMode || vim.visualMode) return false
        // Normal mode: Vim's own Esc, and nothing else.
        cm.state.vimPlugin?.handleKey(e, view)
        e.preventDefault()
        return true
      },
    }),
  )
}
