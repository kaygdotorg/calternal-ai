/**
 * The selection of one bind (docs/description-contract.md 3.2 and section 5.0). `paint(id)` is the cursor moving: it draws at once and sends nothing, however fast the key repeats. `settle(id)` is the rest after a
 * burst (or a click): it sends ONE settled-selection request and accepts the host's `SelectionContextResult` only if it is for exactly the row now drawn, on this lease and partition and not superseded. `flush()` is
 * what an activation calls first: a selection still held by the settle window is sent now, and the activation waits for the host-issued context; `contextFor` returns a context only when it matches the drawn row, so
 * an action never runs against the previously settled row. Contexts live in memory only.
 */
import type { ActionContexts } from "../model";
import type { HostSession } from "../wire/session";

/** The part of a session a selection needs. */
export type SelectionHost = Pick<HostSession, "selection" | "lease">;

/** The selection of one bind. */
export class Selection {
  /** The row drawn as selected (the cursor), or null. */
  drawn = $state<string | null>(null);
  /** Settled selection requests sent (tests and the harness read it). */
  sent = 0;
  #contexts = $state<ActionContexts | null>(null);
  #contextsRow: string | null = null;
  #settled: string | null = null;
  #seq = 0;
  #pending: Promise<void> | null = null;

  constructor(readonly bind: string, private host: SelectionHost) {}

  /** The cursor moved to `id`: drawn at once, nothing sent. */
  paint(id: string): void { this.drawn = id; }

  /** The cursor rested on `id` (or it was clicked): one request, unless that row is already the settled one. */
  settle(id: string): Promise<void> {
    this.drawn = id;
    if (id === this.#settled) return this.#pending ?? Promise.resolve();
    const seq = ++this.#seq;
    this.#settled = id; this.#contexts = null; this.#contextsRow = null; this.sent++;
    const lease = this.host.lease;
    this.#pending = this.host.selection(this.bind, id).then((r) => {
      const now = this.host.lease;
      // Only the answer for the latest settled row, bound to this lease and partition, counts; anything else is ignored.
      if (seq !== this.#seq || r.row !== id || r.bind !== this.bind || !lease || !now || r.lease_ref !== lease.lease_ref || r.cache_partition !== lease.cache_partition) return;
      this.#contexts = r.activation_contexts; this.#contextsRow = id;
    }, () => { if (seq === this.#seq) this.#settled = null; });
    return this.#pending;
  }

  /** Before an activation: sends a selection the settle window still holds and waits until the host has answered for the drawn row. */
  async flush(): Promise<void> { if (this.drawn !== null) await this.settle(this.drawn); }

  /** The context for `action` on the drawn row, or undefined when none was issued for exactly that row (the activation then waits and sends nothing). */
  contextFor(action: string): string | undefined { return this.#contextsRow !== null && this.#contextsRow === this.drawn ? this.#contexts?.[action] : undefined; }

  /** Forgets the selection (the list's result set was replaced and the row is gone). */
  clear(): void { this.drawn = null; this.#settled = null; this.#contexts = null; this.#contextsRow = null; this.#seq++; }
}
