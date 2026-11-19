/**
 * The small things drawn in place of hidden markdown: a bullet for `-`, a
 * checkbox for `[ ]`. (`---` lines are drawn by breaks.ts.) They are plain,
 * unstyled elements with stable class names; the look comes from the app's stylesheet.
 */

import { type EditorView, WidgetType } from '@codemirror/view'
import { toggleTask } from '../transforms/tasks'

export class BulletWidget extends WidgetType {
  override eq(): boolean {
    return true
  }
  toDOM(): HTMLElement {
    const el = document.createElement('span')
    el.className = 'cm-bullet'
    el.setAttribute('aria-hidden', 'true')
    el.textContent = '•'
    return el
  }
}

/** Task states as written in markdown: `[ ]`, `[x]`, `[/]` (doing), `[-]` (cancelled). */
export type TaskState = 'todo' | 'done' | 'doing' | 'cancelled'

export function taskState(ch: string): TaskState {
  if (ch === 'x' || ch === 'X') return 'done'
  if (ch === '/') return 'doing'
  if (ch === '-') return 'cancelled'
  return 'todo'
}

const ARIA: Record<TaskState, string> = { todo: 'false', done: 'true', doing: 'mixed', cancelled: 'false' }

/** The tick and the cancelled dash, drawn as strokes so the CSS can draw them in (the spring check, #42). Parsed once, then cloned per box. */
let template: HTMLElement | null = null
function box(): HTMLElement {
  if (!template) {
    template = document.createElement('span')
    // A checkbox, and a Tab stop (the global keyboard ring in base.css draws its focus).
    template.setAttribute('role', 'checkbox')
    template.tabIndex = 0
    template.innerHTML =
      '<svg viewBox="0 0 24 24" aria-hidden="true" class="cm-task-mark"><path class="cm-task-tick" pathLength="1" d="M6 12.6l4 4L18.2 8.3"/><path class="cm-task-dash" pathLength="1" d="M6.5 12h11"/></svg>'
  }
  return template.cloneNode(true) as HTMLElement
}

function paint(el: HTMLElement, state: TaskState, at: number) {
  el.className = `cm-task cm-task-${state}`
  el.setAttribute('aria-checked', ARIA[state])
  el.dataset.at = String(at)
  // Keep the name the box has (it carries the words); only the kind may have changed.
  const words = el.dataset.words
  const kind = state === 'cancelled' ? 'Cancelled task' : 'Task'
  el.setAttribute('aria-label', words ? `${kind}: ${words}` : kind)
}

/**
 * Name the box after its task: "Task: Renew passport". The words are the line's text after `[ ]`, with
 * link syntax and the inline marks dropped (a link reads as its text). One line, read only when the box
 * gets focus, so the typing path is untouched. Long words are cut for a screen reader.
 */
function nameFromText(view: EditorView, el: HTMLElement) {
  const at = Number(el.dataset.at)
  const line = view.state.doc.lineAt(at)
  el.dataset.words = line.text
    .slice(at - line.from + 2)
    .replace(/!?\[([^\]]*)\]\([^)]*\)|[*_~`=]|\[\[|\]\]/g, '$1')
    .trim()
    .slice(0, 80)
  paint(el, taskState(view.state.doc.sliceString(at, at + 1)), at)
}

export class CheckboxWidget extends WidgetType {
  constructor(
    readonly state: TaskState,
    /** Position of the character inside the brackets. */
    readonly at: number,
  ) {
    super()
  }
  override eq(other: CheckboxWidget): boolean {
    return other.state === this.state && other.at === this.at
  }
  toDOM(view: EditorView): HTMLElement {
    const el = box()
    // Space and Enter flip it without leaving it; preventDefault keeps the editor's own Enter (a new list line) from running.
    el.dataset.at = String(this.at)
    nameFromText(view, el)
    el.addEventListener('keydown', (ev) => {
      if (ev.key !== ' ' && ev.key !== 'Enter') return
      ev.preventDefault()
      toggleTask(view, el, false)
    })
    // The name is the task's words, read when the box gets focus (not on every keystroke: the typing path stays untouched).
    el.addEventListener('focus', () => nameFromText(view, el))
    return el
  }
  /**
   * A new state keeps the same element, so the box can move from one look to the next (the CSS
   * transitions). Only a press (`data-task-press` on the editor, set by taskToggle) plays the
   * motion; any other change (keys, another device, a paste) switches at once.
   */
  override updateDOM(dom: HTMLElement, view: EditorView): boolean {
    if (!dom.classList.contains('cm-task')) return false
    const pressed = view.dom.hasAttribute('data-task-press')
    paint(dom, this.state, this.at)
    if (!pressed) {
      dom.classList.add('cm-task-instant')
      void dom.offsetWidth // let the new look be computed without transitions, then allow them again
      dom.classList.remove('cm-task-instant')
    }
    return true
  }
  override ignoreEvent(): boolean {
    return false
  }
}
