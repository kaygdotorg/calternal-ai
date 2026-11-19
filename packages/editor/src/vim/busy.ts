/**
 * Whether Vim is at work right now: handling a key, or running a command line (#538). Its edits
 * then are Vim's, not typing (glue.ts). It lasts until the current task's work is done.
 */

let busy = false

export const vimBusy = (): boolean => busy

/** Marks Vim as at work until the current key or command is done. */
export function markBusy(): void {
  busy = true
  queueMicrotask(() => (busy = false))
}
