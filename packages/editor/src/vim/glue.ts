/**
 * What ties Vim to one editor (#538), set up once Vim has made its CodeMirror 5 stand-in for it:
 *
 * - the mode on the editor (`data-vim`: normal, insert, visual, replace), for the caret's shape;
 * - the command line and messages go to the app (command-line.ts);
 * - the hidden properties are out of reach: Vim's first line is the first one you can see;
 * - one undo step per Vim change: an insert from `i` to Esc, a `dd`, a `cw` with what was typed;
 * - a change from another device while you type in insert mode is theirs, never part of what `.`
 *   repeats;
 * - Vim's own edits never count as typing, so they never open the `[[` picker or a transform;
 * - the block caret follows the text when a pane moves it, and covers the letter exactly (blockfit.ts).
 */

import { type Annotation, EditorSelection, EditorState, Transaction } from '@codemirror/state'
import { type EditorView, ViewPlugin, type ViewUpdate } from '@codemirror/view'
import { Vim } from '@replit/codemirror-vim'
import type * as Y from 'yjs'
import { isRemote } from '../origins'
import { frontmatterEnd, frontmatterHidden } from '../syntax/frontmatter'
import { noteUndo } from '../undo'
import { markBusy, vimBusy } from './busy'
import { bridgeDialogs } from './command-line'
import { fitBlockCursor } from './blockfit'
import { type CM, getCM } from './commands'
import type { VimHost } from './host'

/** Y's default: changes closer than this join one undo step (editor.ts's manager uses it). */
const PLAIN_CAPTURE = 500

/** Vim's own edits, marked as Vim's, kept out of the hidden properties. */
export const vimEdits = EditorState.transactionFilter.of((tr) => {
  if (!vimBusy()) return tr
  const typing = tr.docChanged && tr.isUserEvent('input.type')
  const sel = tr.selection?.main
  const body = frontmatterHidden(tr.state) ? frontmatterEnd(tr.state) : 0
  const hidden = !!sel && body > 0 && (sel.head < body || sel.anchor < body)
  if (!typing && !hidden) return tr
  // Rebuilt rather than added to: the first user event a transaction carries is the one it reports.
  const kept = (tr as unknown as { annotations: readonly Annotation<unknown>[] }).annotations.filter((a) => a.type !== Transaction.userEvent)
  return {
    changes: tr.changes,
    selection: hidden ? EditorSelection.single(Math.max(sel!.anchor, body), Math.max(sel!.head, body)) : tr.selection,
    effects: tr.effects,
    scrollIntoView: tr.scrollIntoView,
    annotations: typing ? [...kept, Transaction.userEvent.of('vim')] : [...kept, ...(tr.annotation(Transaction.userEvent) ? [Transaction.userEvent.of(tr.annotation(Transaction.userEvent)!)] : [])],
  }
})

export const vimGlue = (host: VimHost) =>
  ViewPlugin.define((view: EditorView) => {
    const cm = getCM(view) as unknown as CM | null
    let um: Y.UndoManager | null = null
    if (!cm) return {}
    cm.state.calternalHost = host

    const mode = () => {
      const v = cm.state.vim
      view.dom.dataset.vim = !v ? 'off' : v.insertMode ? (cm.state.overwrite ? 'replace' : 'insert') : v.visualMode ? 'visual' : 'normal'
    }
    mode()
    // Writing comes first: a note opens in insert mode, and Esc gives normal mode. A note shown
    // read-only first (a preview) opens in insert mode once it can be written in.
    const write = () =>
      queueMicrotask(() => {
        if (cm.state.vim && !cm.state.vim.insertMode && !cm.state.vim.visualMode && !view.state.readOnly) Vim.handleKey(cm as never, 'i', 'mapping')
      })
    write()

    // The first line you can see: below the hidden properties in the default mode.
    const firstLine = () => (frontmatterHidden(view.state) ? view.state.doc.lineAt(frontmatterEnd(view.state)).number - 1 : 0)
    cm.firstLine = firstLine
    bridgeDialogs(cm, firstLine)

    // A remote change during insert mode moves Vim's marks, but isn't recorded for `.`.
    const onChange = cm.onChange as (u: ViewUpdate) => void
    cm.onChange = (u: ViewUpdate) => {
      if (cm.state.vim?.insertMode && u.transactions.every((t) => !t.docChanged || isRemote(t))) {
        for (const id in cm.marks) cm.marks[id].update(u.changes)
        return
      }
      onChange(u)
    }

    // Undo steps follow Vim: nothing joins by time; a step ends before each key in normal mode,
    // and when insert mode ends.
    const track = (next: Y.UndoManager | null) => {
      if (um === next) return
      if (um) um.captureTimeout = PLAIN_CAPTURE
      um = next
      if (um) um.captureTimeout = Number.MAX_SAFE_INTEGER
    }
    track(view.state.facet(noteUndo))
    const cut = () => um?.stopCapturing()
    cm.on('inputEvent', (e: { type?: string }) => {
      if (e?.type !== 'handleKey') return
      markBusy()
      if (!cm.state.vim?.insertMode) cut()
    })
    cm.on('vim-mode-change', () => {
      if (!cm.state.vim?.insertMode) cut()
      mode()
    })
    cm.on('vim-command-done', mode)

    // The block caret is placed by measuring, once per change. A pane sliding or resizing around
    // the editor moves the text without changing it: measure again when that motion ends.
    queueMicrotask(() => fitBlockCursor(cm, view))
    const redraw = () => cm.state.vimPlugin?.blockCursor?.scheduleRedraw()
    document.addEventListener('transitionend', redraw)
    addEventListener('resize', redraw)

    return {
      update(u: ViewUpdate) {
        fitBlockCursor(cm, view)
        if (u.startState.readOnly && !u.state.readOnly) write()
        track(u.state.facet(noteUndo))
        // A change not made by a Vim key while in normal mode (the block bar, a menu) is a step of its own.
        if (u.docChanged && !vimBusy() && !cm.state.vim?.insertMode && u.transactions.some((t) => t.docChanged && !isRemote(t))) cut()
      },
      destroy() {
        document.removeEventListener('transitionend', redraw)
        removeEventListener('resize', redraw)
        track(null)
        delete view.dom.dataset.vim
        delete cm.state.calternalHost
      },
    }
  })
