/**
 * Vim's command line in the app's bar (#538). Vim asks its editor for a prompt (`openDialog`) for
 * `:`, `/` and `?`, and for a message (`openNotification`). Here both go to the app: the prompt
 * becomes a `CommandLine` the bottom bar types into, the message a short notice. The behaviour is
 * Vim's own dialog's: Enter runs the line, Esc drops it, losing focus drops it, and the line's own
 * key handler (search history, Esc in a search) goes first.
 *
 * In the default mode the properties are hidden, so line numbers count from the first line you
 * can see: `:1` is the title, and `%` is the note's body (see `firstLine` in glue.ts).
 */

import { markBusy } from './busy'
import type { CM } from './commands'
import { hostOf } from './commands'
import type { CommandLine } from './host'

interface DialogOptions {
  value?: string
  onKeyDown?: (e: KeyboardEvent, value: string, close: (v?: string) => void) => boolean | void
  onKeyUp?: (e: KeyboardEvent, value: string, close: (v?: string) => void) => void
  onClose?: (dialog: unknown) => void
}

/** The prompt's prefix, from the template Vim built (`<span>:<input></span>`). */
const prefixOf = (template: Element): string => (template.querySelector('span')?.firstChild?.textContent ?? template.textContent ?? '').trim()

/** `:12` and `:3,5s/…` count lines from the first one you can see. */
function shiftLines(input: string, first: number): string {
  if (!first) return input
  return input.replace(/^(\s*:*\s*)(\d+)(\s*,\s*(\d+))?/, (_m, lead: string, a: string, tail: string | undefined, b: string | undefined) => {
    const end = tail && b ? tail.replace(b, String(Number(b) + first)) : ''
    return `${lead}${Number(a) + first}${end}`
  })
}

export function bridgeDialogs(cm: CM, firstLine: () => number): void {
  cm.openDialog = (template: Element, callback: ((value: string) => void) | undefined, options: DialogOptions = {}) => {
    const host = hostOf(cm)
    const prefix = prefixOf(template)
    let closed = false
    let line: CommandLine
    const close = (value?: string) => {
      if (typeof value === 'string') return line.onValue?.(value)
      if (closed) return
      closed = true
      host?.commandLine(null)
      cm.cm6.focus()
      options.onClose?.(template)
    }
    line = {
      prefix,
      value: options.value ?? '',
      keydown(e, value) {
        if (closed || options.onKeyDown?.(e, value, close)) return
        if (e.key === 'Enter') {
          e.preventDefault()
          close()
          markBusy()
          callback?.(prefix === ':' ? shiftLines(value, firstLine()) : value)
        } else if (e.key === 'Escape') {
          e.preventDefault()
          close()
        }
      },
      keyup(e, value) {
        if (!closed) options.onKeyUp?.(e, value, close)
      },
      blur: () => close(),
      onValue: null,
    }
    host?.commandLine(line)
    return close
  }

  cm.openNotification = (template: Element) => {
    const text = template.textContent?.replace('Press ENTER or type command to continue', '').trim()
    if (text) hostOf(cm)?.say(text)
    return () => {}
  }
}
