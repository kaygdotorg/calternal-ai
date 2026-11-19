/**
 * Vim's commands, fitted to Calternal (#538). Vim keeps its keymap and commands once per page,
 * so this runs once, when Vim mode is first loaded; what each command acts on comes from the
 * editor it runs in (`hostOf`).
 *
 * - `u` and Ctrl-r undo and redo this person's own changes, as ⌘Z does (ADR 0022), never someone
 *   else's: the note's history is the shared text's, not CodeMirror's.
 * - `V` selects whole blocks, the app's block selection, not lines (`blockKeys` in keys.ts walks it).
 * - `:w` reports the receipt-backed save status, `:q` leaves the note, `:wq` and `:x` both.
 * - `it` and `at` find inline HTML tags in markdown, which has no tag tree of its own.
 */

import type { EditorView } from '@codemirror/view'
import { CodeMirror, Vim, getCM } from '@replit/codemirror-vim'
import { yUndoManagerKeymap } from '../undo'
import { blocksIn } from '../blocks/blocks'
import type { VimHost } from './host'
import { tagAround } from './tags'
import { describe, editorSaveStatus } from '../sync/status'

/** What Vim calls an editor: CodeMirror 5's interface over our view. */
export type CM = { cm6: EditorView; state: Record<string, any>; [k: string]: any }

export const hostOf = (cm: CM): VimHost | undefined => cm.state.calternalHost

function history(view: EditorView, key: 'Mod-z' | 'Mod-y'): void {
  yUndoManagerKeymap.find((b) => b.key === key)?.run?.(view)
}

/** `V`: the blocks under the caret, or under a character selection. */
function selectBlocks(cm: CM): void {
  const view = cm.cm6
  const host = hostOf(cm)
  if (!host) return
  const sel = view.state.selection.main
  const spans = blocksIn(view.state, sel.from, sel.to)
  if (!spans.length) return
  // Leave Vim's own visual mode first: from here the selection is the app's.
  if (cm.state.vim?.visualMode) Vim.exitVisualMode(cm as never, true)
  host.selectBlocks(view, spans, 'end')
}

let ready = false

export function setUpVim(): void {
  if (ready) return
  ready = true
  const cmds = CodeMirror.commands as Record<string, unknown>
  cmds.undo = (cm: CM) => history(cm.cm6, 'Mod-z')
  cmds.redo = (cm: CM) => history(cm.cm6, 'Mod-y')
  cmds.save = (cm: CM) => {
    const status = cm.cm6.state.facet(editorSaveStatus)?.()
    hostOf(cm)?.say(status ? describe(status) : 'Not saved yet')
  }
  // The `:` commands that held on to the old ones.
  Vim.defineEx('undo', 'u', (cm: any) => history((cm as CM).cm6, 'Mod-z'))
  Vim.defineEx('redo', 'red', (cm: any) => history((cm as CM).cm6, 'Mod-y'))
  const quit = (cm: any) => void hostOf(cm as CM)?.quit((cm as CM).cm6)
  Vim.defineEx('quit', 'q', quit)
  Vim.defineEx('wq', 'wq', quit)
  Vim.defineEx('xit', 'x', quit)

  Vim.defineAction('calternalBlocks', (cm: any) => selectBlocks(cm as CM))
  Vim.mapCommand('V', 'action', 'calternalBlocks', {}, { context: 'normal' })
  Vim.mapCommand('V', 'action', 'calternalBlocks', {}, { context: 'visual' })

  const own = CodeMirror.findEnclosingTag
  ;(CodeMirror as unknown as { findEnclosingTag: unknown }).findEnclosingTag = (cm: CM, pos: { line: number; ch: number }) =>
    own(cm as never, pos as never) ?? tagAround(cm, pos)
}

export { getCM }
