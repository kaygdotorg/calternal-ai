/**
 * Whether the person's words are safe, told honestly.
 *
 * "Saved" only ever means the server said so. A change that lives only in
 * this browser is "not saved yet", with the time it has been waiting, so
 * people can tell a two-second hiccup from a day offline.
 */

import { Facet } from '@codemirror/state'

export type SaveState =
  /** The server has confirmed this device's edits. */
  | 'saved'
  /** Connected; the server hasn't confirmed this device's latest changes yet. */
  | 'saving'
  /** Not connected and some changes exist only on this device. */
  | 'not-saved'

export interface SaveStatus {
  state: SaveState
  /** When the oldest unconfirmed change was made (ms since epoch), if any. */
  since: number | null
  connected: boolean
  /** A change is larger than the server accepts: it can't be saved as it is. */
  tooLarge?: boolean
  error?: string
}

/** Commands read the same authority-backed status as the app's status line. */
export const editorSaveStatus = Facet.define<() => SaveStatus, (() => SaveStatus) | null>({
  combine: (values) => values[0] ?? null,
})

type Listener = (s: SaveStatus) => void

export class StatusBox {
  private value: SaveStatus = { state: 'not-saved', since: null, connected: false }
  private listeners = new Set<Listener>()

  get(): SaveStatus {
    return this.value
  }

  /** Presentation only: receipts.ts supplies every durability decision. */
  set(next: SaveStatus): void {
    const v = { ...next }
    if (v.state === this.value.state && v.since === this.value.since &&
        v.connected === this.value.connected && v.tooLarge === this.value.tooLarge &&
        v.error === this.value.error) return
    this.value = v
    this.listeners.forEach((f) => f(v))
  }

  subscribe(fn: Listener): () => void {
    this.listeners.add(fn)
    fn(this.value)
    return () => this.listeners.delete(fn)
  }
}

/** Plain words for the status line, e.g. "Not saved yet · 3 min". */
export function describe(s: SaveStatus, now = Date.now()): string {
  if (s.state === 'saved') return 'Saved'
  if (s.tooLarge) return 'Too large to save'
  if (s.error) return s.error
  if (s.state === 'saving') return 'Saving…'
  const age = s.since == null ? '' : ` · ${ago(now - s.since)}`
  return `Not saved yet${age}`
}

function ago(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000))
  if (s < 60) return `${s} s`
  const m = Math.round(s / 60)
  if (m < 60) return `${m} min`
  const h = Math.round(m / 60)
  if (h < 48) return `${h} h`
  return `${Math.round(h / 24)} days`
}
