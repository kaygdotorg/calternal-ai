/**
 * An in-process host that speaks the catalogue's sealed wire shapes over a deterministic list, for the unit suites, the harness page and the performance measures. It is NOT the host and decides nothing about
 * authority; it answers view open, windows (start and end anchors, exact or absent count, empty, refine_query, typed refusals on request), selection contexts, lease renewal and leaves the rest to its callers.
 * A test supplies `rows`, `fields` and hooks to make it refuse or change version.
 */
import type { SubscriptionLease, WindowAudience, WindowRequestPayload, WindowReply } from "../src/model";
import type { Route, Transport } from "../src/wire/transport";

/** How the mock behaves. */
export interface MockOptions {
  /** Rows in the list. */
  rows?: number;
  /** `exact` (default) or `none`: whether replies carry a count. */
  count?: "exact" | "none";
  /** Milliseconds each window answer takes. */
  latencyMs?: number;
  /** Fields of row `i`. */
  fields?: (i: number, bind: string) => Record<string, unknown>;
  /** Rows of a particular bind (an item bind has one); default `rows`. */
  rowsFor?: (bind: string) => number;
  /** Bind ids whose rows carry `parent_refs` for each (a description with child binds or repeated sections). */
  parentBinds?: string[];
  /** Action ids that get a host-issued context on every row (default: `open` only). */
  actionIds?: string[];
  /** Called with each window request (a spy). */
  onWindow?: (req: WindowRequestPayload) => void;
  /** Answers `view.renew` (the default retains nothing and renews the same lease). */
  renew?: (body: { renewal_id: string; lease_ref: string; cache_partition: string }) => unknown | Promise<unknown>;
  /** Lets a test refuse or replace a window reply; return undefined for the normal answer. */
  intercept?: (req: WindowRequestPayload, n: number) => WindowReply | undefined | Promise<WindowReply | undefined>;
  audience?: WindowAudience;
  /** Answers any route the mock does not (activation, choice, status, undo): return the reply, or throw to simulate a dropped connection. */
  handle?: (route: Route, body: unknown) => unknown | Promise<unknown>;
  /** Refuse with `skip_limit` a start-anchored request whose `from`, or an end-anchored one whose distance, is beyond this (contiguous continuations from the last row stay allowed). */
  skipLimit?: number;
  /** Refuse every end anchor with `end_unavailable`. */
  endUnavailable?: boolean;
}

/** The in-process host. */
export class MockHost implements Transport {
  rows: number;
  version = "rv-1";
  windowCalls = 0;
  readonly opts: MockOptions;
  constructor(o: MockOptions = {}) { this.opts = o; this.rows = o.rows ?? 100; }
  readonly lease: SubscriptionLease = { lease_ref: "l-1", lease_seconds: 15, grant_revision: "g-1", cache_partition: "cp-1" };
  readonly log: { route: Route; body: unknown }[] = [];

  /** When set, the ids of the rows in order (a list that gains and loses rows); `rows` should match its length. */
  order: string[] | null = null;
  /** The id of row `i`. */
  id(i: number): string { return this.order?.[i] ?? `r-${i}`; }
  /** Fields of row `i`. */
  fields(i: number, bind = ""): Record<string, unknown> { return this.opts.fields ? this.opts.fields(i, bind) : { title: `Row ${i}`, updated: 1_700_000_000 - i }; }
  /** The contexts the host would issue for row `i`. */
  contexts(i: number) { return { row: this.id(i), contexts: Object.fromEntries((this.opts.actionIds ?? ["open"]).map((a) => [a, `ctx-${a}-${this.id(i)}`])) }; }

  #last: Record<string, number> = {};
  lastEnd(anchor?: string): number { return this.#last[anchor ?? "start"] ?? -1; }
  setLast(req: WindowRequestPayload) { this.#last[req.anchor ?? "start"] = req.to; }
  /** A normal answer to a window request over this list. */
  answer(req: WindowRequestPayload): WindowReply {
    const n = this.opts.rowsFor ? this.opts.rowsFor(req.bind) : this.rows;
    const audience = this.opts.audience ?? "own_space_member";
    const exact = (this.opts.count ?? "exact") === "exact";
    if (n === 0 && req.from === 0) return { audience, bind: req.bind, state: "empty", result_version: this.version, offset: 0, rows: [], window_ref: `w-${req.bind}`, ...(exact ? { count: { mode: "exact", value: 0 } } : {}) } as WindowReply;
    const span = Math.min(200, req.to - req.from);
    const plan = (reason: string) => ({ status: 422, code: "unsupported_query_plan", reason }) as WindowReply;
    if (req.anchor === "end" && (!exact || this.opts.endUnavailable)) return plan("end_unavailable");
    if (this.opts.skipLimit !== undefined && req.from > this.opts.skipLimit && req.from !== this.lastEnd(req.anchor)) return plan("skip_limit");
    this.setLast(req);
    if (req.anchor === "end") {
      // distance zero is the last row; the rows come back in normal order, so the window is [last-to+1, last-from]
      const hi = n - 1 - req.from, lo = Math.max(0, hi - span + 1);
      const idx: number[] = []; for (let i = lo; i <= hi; i++) idx.push(i);
      return { audience, bind: req.bind, state: "ready", result_version: this.version, offset: n - 1 - hi, anchor: "end", rows: idx.map((i) => ({ id: this.id(i), fields: this.fields(i, req.bind), ...this.refs(i) })), window_ref: `w-${req.bind}-e`, count: { mode: "exact", value: n }, activation_contexts: idx.map((i) => this.contexts(i)) } as WindowReply;
    }
    const idx: number[] = []; for (let i = req.from; i < Math.min(n, req.from + span); i++) idx.push(i);
    return { audience, bind: req.bind, state: "ready", result_version: this.version, offset: req.from, rows: idx.map((i) => ({ id: this.id(i), fields: this.fields(i, req.bind), ...this.refs(i) })), window_ref: `w-${req.bind}-${req.from}`, ...(exact ? { count: { mode: "exact", value: n } } : {}), activation_contexts: idx.map((i) => this.contexts(i)) } as WindowReply;
  }

  /** The parent refs every row carries when the description has child binds. */
  refs(i: number) { return this.opts.parentBinds?.length ? { parent_refs: Object.fromEntries(this.opts.parentBinds.map((b) => [b, `p-${b}-${i}`])) } : {}; }

  async post<T>(route: Route, body: unknown): Promise<T> {
    this.log.push({ route, body });
    if (route === "view.open") return { lease: this.lease, activation_contexts: {}, audience: this.opts.audience ?? "own_space_member" } as T;
    if (route === "view.renew" && this.opts.renew) return (await this.opts.renew(body as never)) as T;
    if (route === "view.renew") { const b = body as { renewal_id: string }; return { lease: this.lease, retained_results: [], activation_contexts: {}, windows: [], renewal_id: b.renewal_id, request_lease_ref: this.lease.lease_ref, cache_partition: this.lease.cache_partition } as T; }
    if (route === "selection.settled") { const b = body as { bind: string; row: string }; return { type: "selection_context", lease_ref: this.lease.lease_ref, cache_partition: this.lease.cache_partition, bind: b.bind, row: b.row, activation_contexts: { open: `ctx-${b.row}` } } as T; }
    if (route === "window") {
      const req = (body as { request: WindowRequestPayload }).request; const n = ++this.windowCalls;
      this.opts.onWindow?.(req);
      if (this.opts.latencyMs) await new Promise((r) => setTimeout(r, this.opts.latencyMs));
      const over = await this.opts.intercept?.(req, n);
      return (over ?? this.answer(req)) as T;
    }
    if (this.opts.handle) return (await this.opts.handle(route, body)) as T;
    throw new Error(`The mock host has no ${route}.`);
  }
}
