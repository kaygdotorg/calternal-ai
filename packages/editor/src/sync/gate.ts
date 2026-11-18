/**
 * Holds remote changes back while someone is mid-word with an input method.
 *
 * Phone keyboards (autocorrect, Japanese, Chinese, Android's composing
 * underline) keep a word "open" while you type it. Changing the text under
 * an open composition breaks it: letters double, vanish or jump. So while a
 * composition is open, changes from other devices and tabs wait here and
 * are applied the moment it closes.
 */

export class ImeGate {
  private open = false
  private queue: Array<() => void> = []

  get composing(): boolean {
    return this.open
  }

  /** Runs `apply` now, or right after the current composition ends. */
  run(apply: () => void): void {
    if (this.open) this.queue.push(apply)
    else apply()
  }

  start(): void {
    this.open = true
  }

  /** Ends the composition and applies everything that waited, in order. */
  end(): void {
    this.open = false
    const waiting = this.queue
    this.queue = []
    waiting.forEach((f) => f())
  }

  get pending(): number {
    return this.queue.length
  }
}
