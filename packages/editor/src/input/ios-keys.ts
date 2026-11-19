/**
 * One key press, one edit, on iPhone and iPad (#582).
 *
 * Safari on iOS doesn't tell the page what Return did. It edits the text
 * itself (a `beforeinput` of `insertLineBreak`, then a changed page) and
 * CodeMirror reads that back as the key, and runs the key's command: here
 * the one that starts the next bullet. Because that edit can fail to come,
 * CodeMirror also sets a 50 ms timer when the key goes down, and when it
 * fires with no edit seen it runs the command anyway.
 *
 * On a busy phone the edit comes later than 50 ms. The command has already
 * run by then (a new bullet), and then Safari's own line break lands on
 * top of it: two returns from one tap, the caret a line below the bullet.
 * The same goes for Backspace and Delete: two characters gone for one tap.
 *
 * So when CodeMirror has run a key's command by itself and Safari's own
 * edit for that press shows up afterwards, this cancels it. The edit in the
 * normal order (Safari's edit first, then the command) is left alone, as is
 * a key with no command here (the browser's edit is then all there is).
 */

import { type Extension } from '@codemirror/state'
import { type EditorView, ViewPlugin } from '@codemirror/view'

/** The edits Safari makes for the keys CodeMirror waits on. */
const EDITS: Record<string, readonly string[]> = {
  Enter: ['insertLineBreak', 'insertParagraph'],
  Backspace: ['deleteContentBackward'],
  Delete: ['deleteContentForward'],
}

/** How long after the key a late edit still counts as that press's. */
const LATE_MS = 1000

interface Press {
  key: string
  at: number
  /** Safari's own edit came (the normal order). */
  edited: boolean
  /** CodeMirror ran the key's command without waiting for it. */
  ran: boolean
}

class IosKeys {
  private press: Press | null = null
  private dom: HTMLElement

  constructor(view: EditorView) {
    this.dom = view.contentDOM
    // After CodeMirror's own key handler (it was added first), so a command that took the key has
    // already said so by the time this looks.
    this.dom.addEventListener('keydown', this.onKey)
    this.dom.addEventListener('beforeinput', this.onEdit, true)
  }

  private onKey = (ev: KeyboardEvent) => {
    if (!EDITS[ev.key]) return
    // A key CodeMirror makes up from what Safari did (it marks them `synthetic`).
    if (!(ev as KeyboardEvent & { synthetic?: boolean }).synthetic) {
      this.press = { key: ev.key, at: Date.now(), edited: false, ran: false }
      return
    }
    const p = this.press
    if (!p || p.key !== ev.key) return
    // Asked a moment later: a handler CodeMirror held back for an update in progress has run by then.
    queueMicrotask(() => {
      if (ev.defaultPrevented && !p.edited) p.ran = true
    })
  }

  private onEdit = (ev: Event) => {
    const p = this.press
    if (!p || !EDITS[p.key]?.includes((ev as InputEvent).inputType)) return
    if (p.ran && Date.now() - p.at < LATE_MS) {
      ev.preventDefault()
      this.press = null
    } else p.edited = true
  }

  destroy() {
    this.dom.removeEventListener('keydown', this.onKey)
    this.dom.removeEventListener('beforeinput', this.onEdit, true)
  }
}

export const iosKeys: Extension = ViewPlugin.fromClass(IosKeys)
