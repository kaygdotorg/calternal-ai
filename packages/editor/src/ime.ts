/**
 * The input-method gate (ADR 0020): while the person is composing (Japanese, Chinese, Korean, accents on a phone), a change that arrives from elsewhere waits and applies when
 * composition ends. Applying it mid-composition corrupts the composition. `hold` runs the function now when idle, or queues it. Queued work runs in order.
 */
export class ImeGate {
  private composing = false;
  private queue: (() => void)[] = [];
  get active(): boolean { return this.composing; }
  begin(): void { this.composing = true; }
  /** Composition ended: run what waited. */
  end(): void {
    this.composing = false;
    const q = this.queue;
    this.queue = [];
    for (const f of q) f();
  }
  hold(f: () => void): void { if (this.composing) this.queue.push(f); else f(); }
  get waiting(): number { return this.queue.length; }
}
