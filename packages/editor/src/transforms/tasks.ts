/**
 * Ticking a task box. A click flips `[ ]` ↔ `[x]` (doing and cancelled go
 * back to open); the change is a single character, made by this person.
 */

import { EditorView } from '@codemirror/view'

const NEXT: Record<string, string> = { ' ': 'x', x: ' ', X: ' ', '/': 'x', '-': ' ' }

const timers = new WeakMap<Element, ReturnType<typeof setTimeout>>()

/**
 * Flip the task whose box is `el`. Shared by the pointer path (below) and the box's own keys
 * (Space/Enter, widgets.ts), so both make the same one-character change. `press` plays the spring
 * check (a pointer press); the keyboard passes false, so it switches at once, with no motion.
 * Returns false when `el` is not a box we can flip.
 */
export function toggleTask(view: EditorView, el: HTMLElement, press: boolean): boolean {
  const at = Number(el.dataset.at)
  const next = NEXT[view.state.doc.sliceString(at, at + 1)]
  if (next == null) return false
  if (press) {
    // Your press plays the spring check; the widgets and the CSS read this flag (see widgets.ts).
    view.dom.setAttribute('data-task-press', '')
    clearTimeout(timers.get(view.dom))
    timers.set(view.dom, setTimeout(() => view.dom.removeAttribute('data-task-press'), 700))
  }
  view.dispatch({ changes: { from: at, to: at + 1, insert: next }, userEvent: 'input.toggle' })
  return true
}

export const taskToggle = EditorView.domEventHandlers({
  mousedown(ev, view) {
    const el = (ev.target as HTMLElement | null)?.closest('.cm-task') as HTMLElement | null
    if (!el || !toggleTask(view, el, true)) return false
    ev.preventDefault()
    return true
  },
})
