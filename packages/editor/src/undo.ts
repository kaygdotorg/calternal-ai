/**
 * The note's undo history, reachable from commands, so an action like
 * "+ Log" can be its own undo step instead of merging with nearby typing.
 */

import { Facet, Prec } from '@codemirror/state'
import { EditorView, type KeyBinding } from '@codemirror/view'
import { yUndoManagerKeymap as sharedHistory } from 'y-codemirror.next'
import type * as Y from 'yjs'

export const noteUndo = Facet.define<Y.UndoManager, Y.UndoManager | null>({
  combine: (v) => v[0] ?? null,
})

/** Checked on the command itself, before a sibling's freeze notification arrives. */
export const editPermission = Facet.define<() => boolean, readonly (() => boolean)[]>({
  combine: (values) => values,
})

// Yjs undo/redo mutates the shared document before CodeMirror's transaction
// filters run. Every keyboard, Vim and app undo command must use this guard.
export const yUndoManagerKeymap: KeyBinding[] = sharedHistory.map((binding) => ({
  ...binding,
  run: (view: EditorView) => {
    if (view.state.readOnly || view.state.facet(editPermission).some((allow) => !allow())) return true
    return binding.run?.(view) ?? false
  },
}))

// yCollab also installs a native history handler that calls Yjs directly.
// Consume those events first, including Edit-menu and mobile undo/redo.
export const historyInput = Prec.highest(EditorView.domEventHandlers({
  beforeinput(event, view) {
    const key = event.inputType === 'historyUndo' ? 'Mod-z' : event.inputType === 'historyRedo' ? 'Mod-y' : null
    if (!key) return false
    event.preventDefault()
    yUndoManagerKeymap.find((binding) => binding.key === key)?.run?.(view)
    return true
  },
}))
