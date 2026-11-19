/**
 * A STAND-IN for the routes the step 0 host does not have: `transcript.page`, `composer.options`, the composer's send, and a request card with its resolve. It wraps another transport and answers only those routes
 * (everything else passes through), with deterministic content per thread ref, a cursor the way the catalogue describes it (opaque, checked, refused when it is not one it issued), and a new committed event every few
 * seconds so the follow-the-end rule can be seen. It is test and demo scaffolding, not the host: nothing here decides authority.
 */
import { HostError, type Route, type Transport } from "../src/wire/transport";

/** What the stand-in is told. */
export interface SyntheticOptions { inner: Transport; events?: number; appendEveryMs?: number; now?: () => number }

type Ev = { event_id: string; kind: string; data: Record<string, unknown> };

export class SyntheticSource implements Transport {
  readonly inner: Transport;
  readonly log: { route: Route; body: unknown }[] = [];
  readonly #threads = new Map<string, Ev[]>();
  readonly #n: number; readonly #every: number; readonly #now: () => number; readonly #t0: number;
  #sends = new Map<string, string>();
  constructor(o: SyntheticOptions) { this.inner = o.inner; this.#n = o.events ?? 240; this.#every = o.appendEveryMs ?? 4000; this.#now = o.now ?? (() => Date.now()); this.#t0 = this.#now(); }

  /** Committed events of a thread now: the seeded history plus one per `appendEveryMs` since the start. */
  #events(thread: string): Ev[] {
    let list = this.#threads.get(thread);
    if (!list) { list = Array.from({ length: this.#n }, (_, i) => ({ event_id: `e-${thread.slice(0, 8)}-${i}`, kind: "message", data: { role: i % 2 ? "assistant" : "user", text: `${i % 2 ? "Answer" : "Question"} ${i + 1} in ${thread.slice(0, 8)}: ${"the quick brown fox ".repeat(1 + (i % 4))}` } })); this.#threads.set(thread, list); }
    const due = Math.floor((this.#now() - this.#t0) / this.#every);
    while (list.length < this.#n + due) list.push({ event_id: `e-${thread.slice(0, 8)}-${list.length}`, kind: "message", data: { role: "assistant", text: `Streamed update ${list.length + 1}` } });
    return list;
  }

  async post<T>(route: Route, body: unknown, opts: { lease?: string; signal?: AbortSignal; route?: Record<string, string> } = {}): Promise<T> {
    const b = body as Record<string, any>;
    if (route === "transcript.page") {
      this.log.push({ route, body });
      const list = this.#events(String(b.thread)), limit = Math.min(128, b.limit ?? 60);
      let from = Math.max(0, list.length - limit);
      if (b.after !== undefined) { const m = /^c(\d+)$/.exec(String(b.after)); if (!m || Number(m[1]) > list.length) throw new HostError(410, "cursor_expired", "That position is no longer valid."); from = Number(m[1]); }
      const events = list.slice(from, from + limit);
      return { events, committed_cursor: `c${from + events.length}` } as T;
    }
    if (route === "composer.options") {
      this.log.push({ route, body });
      const sets: Record<string, string[]> = { provider: ["Claude", "Codex"], model: ["Sonnet 5.5", "Opus 5.5", "Haiku 5.5"], reasoning: ["Low", "Medium", "High"], permission: ["Ask", "Edit files", "Full access"], machine: ["This computer"] };
      const labels = sets[b.control] ?? [];
      return { control: b.control, options: labels.map((l) => ({ ref: `opt-${b.control}-${l.toLowerCase().replace(/\W+/g, "-")}`, label: l })), selected: null, default: labels.length ? `opt-${b.control}-${labels[0]!.toLowerCase().replace(/\W+/g, "-")}` : null } as T;
    }
    if (route === "activation.resolve" && b.action_id === "composer") {
      this.log.push({ route, body });
      const ref = `synthetic-res-${this.#sends.size}`; this.#sends.set(ref, JSON.stringify(b.interaction?.options ?? {}));
      return { activation_id: b.activation_id, resolution_ref: ref } as T;
    }
    if (route === "activation.confirm" && String(b.resolution_ref).startsWith("synthetic-res-")) {
      this.log.push({ route, body });
      if (typeof b.final_text !== "string" || !b.final_text.trim()) return { activation_id: b.activation_id, state: "error", code: "empty", message: "There is nothing to send." } as T;
      return { activation_id: b.activation_id, state: "ok" } as T;
    }
    if (route === "request.card") { this.log.push({ route, body }); return { title: "Run a command", card_context: "synthetic-card", material: [{ label: "Command", value: "npm test" }, { label: "Directory", value: "/workspace" }] } as T; }
    if (route === "request.resolve") { this.log.push({ route, body }); return { activation_id: "synthetic", state: "ok" } as T; }
    return this.inner.post<T>(route, body, opts);
  }
  get image() { return this.inner.image?.bind(this.inner); }
}
