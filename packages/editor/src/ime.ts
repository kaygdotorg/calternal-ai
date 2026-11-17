/**
 * The input-method gate (ADR 0020): while the person is composing (Japanese, Chinese, Korean, accents on a phone), a change that arrives from elsewhere waits and applies when
 * composition ends. Applying it mid-composition corrupts the composition. `hold` runs the function now when idle, or queues it. Queued work runs in order.
 */
export class ImeGate {
  private composing = false;
  private queue: (() => void)[] = [];
  private endedAt = -1e9;
  get active(): boolean { return this.composing; }
  begin(): void { this.composing = true; }
  /** Composition ended: run what waited. */
  end(): void {
    this.composing = false;
    this.endedAt = performance.now();
    const q = this.queue;
    this.queue = [];
    for (const f of q) f();
  }
  /**
   * True while a key event belongs to composition: it says so itself (`isComposing`, or the legacy key code 229), or it is the key that confirmed the composition, which Safari delivers just AFTER
   * `compositionend` with `isComposing` already false (Enter, Escape or Tab would then submit, cancel or move focus in the middle of confirming a word).
   */
  composingKey(e: { isComposing?: boolean; keyCode?: number; key?: string }): boolean {
    if (e.isComposing || e.keyCode === 229) return true;
    return (e.key === "Enter" || e.key === "Escape" || e.key === "Tab") && performance.now() - this.endedAt < 80;
  }
  hold(f: () => void): void { if (this.composing) this.queue.push(f); else f(); }
  get waiting(): number { return this.queue.length; }
}
