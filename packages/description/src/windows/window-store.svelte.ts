/**
 * The windows of one bind, held as a sparse array of rows by absolute index. The list asks `ensure(consumer, first, last)` for what it draws; the store turns that into aligned window requests with a prefetch
 * margin, sends at most 4 requests a second and 4 at a time (docs/description-contract.md 2.3), places each reply by its offset, and keeps one `result_version`: a different version replaces everything, except
 * while a key repeats, when it is held and applied once the selection settles (a reply for the version already drawn is applied at once, so arrowing never runs into a skeleton). It draws exactly what the
 * host sent: no row is added or dropped, and a list whose count is not exact grows by one sentinel row until a short `ready` window ends it, so no total is implied.
 *
 * Cost: `ensure` is O(chunks in view); a reply is O(rows in it); `row(i)` is O(1). The reactive `tick` changes once per applied reply.
 */
import { navHold } from "@calternal/ui";
import type { ActionContexts, Bind, WindowReply, WindowRequestPayload, WindowResult, WindowRow, WindowPlanError } from "../model";
import { isPlanError } from "../model";

/** What a store needs to ask for a window. */
export interface WindowSource { window(request: WindowRequestPayload, signal?: AbortSignal): Promise<WindowReply>; }

/** The states a list can be in (the catalogue's `WindowResult.state`, plus the plan error). */
export type ListState = "loading" | "ready" | "empty" | "error" | "refine_query";

/** Window requests a second (the contract's cap) and requests in flight at once. */
export const MAX_REQUESTS_PER_SECOND = 4;
export const MAX_IN_FLIGHT = 4;

/** Options of a store. */
export interface WindowStoreOptions {
  bind: Bind;
  source: WindowSource;
  /** Host-issued parent ref for a child bind; absent at the root. */
  parent?: string;
  /** Replaceable for tests. */
  now?: () => number;
}

/** The windowed rows of one bind. */
export class WindowStore {
  readonly bind: Bind;
  readonly parent?: string;
  /** Rows per window request (the bind's `window.size`, default 60, at most 200). */
  readonly size: number;
  state = $state<ListState>("loading");
  /** Why the list is in `error` when a typed plan refusal caused it. */
  planError = $state<WindowPlanError["reason"] | null>(null);
  /** Bumps once per applied reply; the rows' readers depend on it. */
  tick = $state(0);
  /** The last `window_ref` and `result_version` the host issued, for lease renewal. */
  windowRef: string | null = null;
  resultVersion: string | null = null;

  readonly #source: WindowSource;
  readonly #now: () => number;
  readonly #rows = new Map<number, WindowRow>();
  readonly #contexts = new Map<string, ActionContexts>();
  #count: { mode: "exact" | "estimate"; value: number } | null = null;
  #ended = false;
  #loadedEnd = 0;
  readonly #visible = new Map<unknown, [number, number]>();
  readonly #inflight = new Set<number>();
  #sent: number[] = [];
  #timer: ReturnType<typeof setTimeout> | null = null;
  #held: { reply: WindowResult; req: WindowRequestPayload } | null = null;
  #unsubHold: (() => void) | null = null;
  #dead = false;
  /** Chunks the host refused with a typed plan error; not asked again until the result set changes or a walk has loaded them. */
  readonly #refused = new Set<number>();
  /** Paging toward a goal after a refusal (docs/description-contract.md 2.3, recovery rule): from the start or the end, or forward from the last loaded row. */
  #walk: { kind: "start" | "end" | "forward"; goal: number } | null = null;
  #walking = false;
  /** Rows loaded contiguously from the end (distance), for an end walk. */
  #tailCover = 0;
  #endUnavailable = false;
  /** Requests sent so far (tests and the harness read it). */
  requests = 0;

  constructor(o: WindowStoreOptions) {
    this.bind = o.bind; this.parent = o.parent; this.#source = o.source; this.#now = o.now ?? (() => performance.now());
    this.size = Math.min(200, Math.max(1, o.bind.window?.size ?? 60));
    this.#unsubHold = navHold.subscribe(() => this.#applyHeld());
  }

  /** Total rows to size the scroll extent: the exact count; else the rows known plus one sentinel until the end is seen. While loading, empty or refused there are none. */
  get length(): number {
    void this.tick;
    if (this.state === "loading" || this.state === "empty" || this.state === "error") return 0;
    if (this.#count?.mode === "exact") return this.#count.value;
    if (this.state === "refine_query") return this.#loadedEnd; // provisional rows only: no sentinel, no extent that implies a total
    return this.#loadedEnd + (this.#ended ? 0 : 1);
  }

  /** Whether the host gave an exact count (the scroll extent is then a total). */
  get exact(): boolean { void this.tick; return this.#count?.mode === "exact"; }
  /** The exact total when the host gave one. */
  get total(): number | null { void this.tick; return this.#count?.mode === "exact" ? this.#count.value : null; }
  /** Rows loaded so far (for tests and the harness). */
  get loaded(): number { return this.#rows.size; }

  /** The row at absolute index `i`, or undefined while it is not loaded (the list draws a skeleton row). */
  row(i: number): WindowRow | undefined { void this.tick; return this.#rows.get(i); }
  /** The index of the row with the host-issued id (the cursor follows its row when the list changes under it), or -1. */
  indexOf(id: string): number { void this.tick; for (const [i, r] of this.#rows) if (r.id === id) return i; return -1; }
  /** The activation contexts the host issued with the window for `rowId`: action id to opaque context. */
  contexts(rowId: string): ActionContexts | undefined { return this.#contexts.get(rowId); }

  /** The list reports which rows it draws; the store loads what is missing within a margin. `consumer` identifies the drawing list (a bind may be drawn twice). */
  ensure(consumer: unknown, first: number, last: number): void {
    if (this.#dead) return;
    if (last <= first) this.#visible.delete(consumer); else this.#visible.set(consumer, [first, last]);
    const w = this.#walk;
    if (w && w.goal !== Number.MAX_SAFE_INTEGER && this.#visible.size && ![...this.#visible.values()].some(([f, l]) => w.goal >= f - this.size * 2 && w.goal < l + this.size * 2)) this.#walk = null; // the person went elsewhere
    this.#pump();
  }

  /** Stops the store: no further requests, replies are dropped. */
  destroy() { this.#dead = true; if (this.#timer) clearTimeout(this.#timer); this.#unsubHold?.(); }

  // ---- requests -------------------------------------------------------------------------------------------------------------------------------

  /** The window starts to load, nearest to what is drawn first. */
  #wanted(): number[] {
    const first = this.state === "loading" && this.#rows.size === 0;
    const limit = this.#count?.mode === "exact" ? this.#count.value : this.#ended ? this.#loadedEnd : first ? this.size : this.#loadedEnd + this.size;
    const want = new Set<number>();
    const centre = [...this.#visible.values()];
    if (this.state === "loading" && this.#rows.size === 0 && !this.#visible.size) want.add(0); // first window even before the list has measured itself
    for (const [f, l] of centre) {
      const a = Math.max(0, f - Math.floor(this.size / 2)), b = Math.min(limit, l + this.size);
      for (let c = Math.floor(a / this.size); c * this.size < b; c++) want.add(c);
    }
    const mid = centre.length ? centre[0]![0] : 0;
    return [...want].filter((c) => !this.#inflight.has(c) && !this.#refused.has(c) && this.#firstMissing(c, limit) >= 0).sort((x, y) => Math.abs(x * this.size - mid) - Math.abs(y * this.size - mid));
  }

  /** The first index in chunk `c` that is not loaded, or -1 when the chunk is whole (or beyond the end). */
  #firstMissing(c: number, limit: number): number {
    const s = c * this.size, e = Math.min(s + this.size, limit);
    for (let i = s; i < e; i++) if (!this.#rows.has(i)) return i;
    return -1;
  }

  #pump() {
    if (this.#dead || this.state === "error" || this.state === "refine_query" || this.state === "empty") return;
    const now = this.#now();
    this.#sent = this.#sent.filter((t) => now - t < 1000);
    this.#pumpWalk();
    for (const c of this.#wanted()) {
      if (this.#inflight.size >= MAX_IN_FLIGHT) return;
      if (this.#sent.length >= MAX_REQUESTS_PER_SECOND) { this.#later(1000 - (now - this.#sent[0]!)); return; }
      this.#send(c);
    }
  }

  #later(ms: number) { if (this.#timer) return; this.#timer = setTimeout(() => { this.#timer = null; this.#pump(); }, Math.max(1, ms)); }

  /** Whether the bind is a search (no end anchor, no exact-count jumps). */
  get #search(): boolean { return this.bind.source.type === "search"; }

  /** A chunk in the tail of an exact-count, non-search list is asked from the end (so End never walks the list); everything else from the start. */
  #endChunk(c: number): boolean {
    const n = this.#count?.mode === "exact" ? this.#count.value : -1;
    return n >= 0 && !this.#search && !this.#endUnavailable && n - Math.min((c + 1) * this.size, n) < 3 * this.size && c * this.size > this.#loadedEnd;
  }

  #send(c: number) {
    const limit = this.#count?.mode === "exact" ? this.#count.value : Infinity;
    const s = c * this.size, e = Math.min(s + this.size, limit);
    let req: WindowRequestPayload;
    if (this.#endChunk(c)) req = { bind: this.bind.id, from: limit - e, to: limit - s, anchor: "end" };
    else { const from = Math.max(s, this.#firstMissing(c, limit)); req = { bind: this.bind.id, from, to: (c + 1) * this.size }; }
    this.#dispatch(req, c, false);
  }

  #dispatch(req: WindowRequestPayload, c: number, walk: boolean) {
    if (this.parent) req.parent = this.parent;
    this.#inflight.add(c); this.#sent.push(this.#now()); this.requests++;
    if (walk) this.#walking = true;
    this.#source.window(req).then((r) => this.#accept(r, req, c, walk), () => this.#failed()).finally(() => { this.#inflight.delete(c); if (walk) this.#walking = false; this.#pump(); });
  }

  /** Whether row `i` is known to exist before the walk's goal is reached. */
  #pumpWalk() {
    const w = this.#walk;
    if (!w || this.#walking) return;
    if (this.#rows.has(w.goal) || (w.goal < 0)) { this.#walk = null; return; }
    if (this.#ended && w.kind !== "end") { this.#walk = null; return; }
    if (this.#inflight.size >= MAX_IN_FLIGHT) return;
    const now = this.#now();
    this.#sent = this.#sent.filter((t) => now - t < 1000);
    if (this.#sent.length >= MAX_REQUESTS_PER_SECOND) { this.#later(1000 - (now - this.#sent[0]!)); return; }
    const key = -1 - this.requests; // walk requests are not chunks
    if (w.kind === "end") this.#dispatch({ bind: this.bind.id, from: this.#tailCover, to: this.#tailCover + this.size, anchor: "end" }, key, true);
    else this.#dispatch({ bind: this.bind.id, from: this.#loadedEnd, to: this.#loadedEnd + this.size }, key, true);
  }

  /** The goal row of a refusal recovery in progress, or null. */
  get walkGoal(): number | null { return this.#walk ? this.#walk.goal : null; }

  /**
   * End and far jumps (section 5.0): with an exact count the list simply scrolls (the tail is asked from the end); without one (search, estimate or none) this pages forward from the last loaded row until a short
   * ready window ends the list, so the target is retained across the pages. A refine_query answer stops it.
   */
  jump(target: "end"): void {
    if (this.#count?.mode === "exact" && !this.#endUnavailable && !this.#search) return;
    if (this.#ended) return;
    this.#walk = { kind: "forward", goal: Number.MAX_SAFE_INTEGER };
    this.#pump();
  }

  #failed() { if (this.#rows.size === 0) this.#set("error", null); }

  // ---- replies --------------------------------------------------------------------------------------------------------------------------------

  #set(state: ListState, plan: WindowPlanError["reason"] | null) { if (this.state !== state) this.state = state; this.planError = plan; }

  #accept(reply: WindowReply, req: WindowRequestPayload, chunk: number, walk: boolean) {
    if (this.#dead) return;
    if (isPlanError(reply)) { this.#refusal(reply.reason, req, chunk, walk); return; }
    if (reply.bind !== this.bind.id) return;
    switch (reply.state) {
      case "loading": case "throttled": this.#set(this.#rows.size ? this.state : "loading", null); this.#later(1000); return;
      case "error": this.#set("error", null); return;
      case "empty":
        // `empty` is only the whole result being empty, and only at the start; a later-page `empty` is refused by the contract and changes nothing.
        if (req.from !== 0) { this.#ended = true; this.#loadedEnd = Math.max(this.#loadedEnd, req.from); this.#bump(); return; }
        this.#clear(); this.#count = { mode: "exact", value: 0 }; this.resultVersion = reply.result_version; this.windowRef = reply.window_ref; this.#set("empty", null); this.#bump(); return;
      case "ready": case "refine_query": break;
    }
    if (reply.state === "ready" && this.resultVersion !== null && reply.result_version !== this.resultVersion) {
      // A new result set replaces the window wholesale, but never in the middle of a key burst.
      if (navHold.held) { this.#held = { reply, req }; return; }
      this.#clear();
    }
    this.#place(reply, req);
  }

  /** The typed refusals: skip_limit pages from the nearer permitted end, end_unavailable pages forward, anything else (or a refusal of a recovery request itself) shows the bind's error state. */
  #refusal(reason: WindowPlanError["reason"], req: WindowRequestPayload, chunk: number, walk: boolean) {
    const target = chunk >= 0 ? chunk * this.size : (this.#walk?.goal ?? 0);
    if (walk || (reason !== "skip_limit" && reason !== "end_unavailable")) { this.#walk = null; this.#set("error", reason); return; }
    this.#refused.add(chunk);
    if (reason === "end_unavailable") { this.#endUnavailable = true; this.#walk = { kind: "forward", goal: Math.max(target, this.#walk?.goal ?? 0) }; }
    else {
      const n = this.#count?.mode === "exact" ? this.#count.value : -1;
      const fromEnd = n >= 0 && !this.#search && !this.#endUnavailable && target > n / 2;
      this.#walk = { kind: fromEnd ? "end" : "start", goal: target };
    }
    this.#pump();
  }

  #applyHeld() { const h = this.#held; if (!h) return; this.#held = null; this.#clear(); this.#place(h.reply, h.req); this.#pump(); }

  #clear() { this.#walk = null; this.#refused.clear(); this.#tailCover = 0; this.#rows.clear(); this.#contexts.clear(); this.#count = null; this.#ended = false; this.#loadedEnd = 0; }

  #place(reply: WindowResult, req: WindowRequestPayload) {
    if (reply.state === "ready") { this.resultVersion = reply.result_version; this.windowRef = reply.window_ref; }
    if (reply.count) this.#count = reply.count;
    let base = reply.offset;
    if (reply.anchor === "end" && this.#count?.mode === "exact") base = this.#count.value - 1 - reply.offset - (reply.rows.length - 1);
    reply.rows.forEach((r, k) => this.#rows.set(base + k, r as WindowRow));
    for (const rc of reply.activation_contexts ?? []) this.#contexts.set(rc.row, rc.contexts);
    this.#loadedEnd = Math.max(this.#loadedEnd, base + reply.rows.length);
    if (reply.anchor === "end" && reply.offset <= this.#tailCover) this.#tailCover = Math.max(this.#tailCover, reply.offset + reply.rows.length);
    // A short ready window ends a list whose size is not known; with an exact count the count is the end.
    if (reply.state === "ready" && req.anchor !== "end" && reply.rows.length < req.to - req.from) { this.#ended = true; this.#loadedEnd = Math.max(base + reply.rows.length, this.#loadedEnd); }
    if (reply.state === "refine_query" || (this.#ended && this.#walk && this.#walk.kind !== "end")) this.#walk = null; // refine_query stops any End or forward paging; the end of the list completes a forward walk
    this.#set(reply.state === "refine_query" ? "refine_query" : "ready", null);
    this.#bump();
  }

  #bump() { this.tick++; }
}
