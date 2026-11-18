/**
 * The Threads app's client on the real server: `ThreadsClient` (apps/threads/client.ts) over the orchestrator's operations (`threads.list/get/create/send/stop/rename/archive/unarchive/delete`,
 * docs/threads.md), the space's live journal (`GET /api/v1/live`) and the job event stream (`GET /api/v1/jobs/<id>/events`, docs/agent-turns.md). Selected with `?server=1` (see
 * `store.svelte.ts`); the fixtures stay the default.
 *
 *   - Nothing is polled. One live subscription per space (`SpaceFeed`) tells the client what changed: a turn's state is applied from the entry itself (`threads.turn_state_changed` carries
 *     `data.state` and `data.job_id`); a created, renamed, archived or deleted thread re-reads the list (one `threads.list`, no file read on the server); a saved message or checkpoint of the
 *     open thread re-reads that thread. Entries are coalesced for 120 ms, so a burst of checkpoints is one read. A reset re-reads everything and subscribes again.
 *   - The list says everything the sidebar shows (turn state, provider, model, mode, message count, last activity, archived, parent). The open thread's messages come from its Markdown
 *     (`threads.get`), parsed once per revision and read in ranges from that parse.
 *   - A running turn is followed on its job stream for steps and the working state. `threads.get` names the running job, so a reloaded page follows the turn it finds there.
 *   - Not on the server: where a thread runs (the server picks the machine; the choice is kept here) and the request operations (#32): `requests.list` answers not_found and the list stays empty.
 *     The permission mode, provider and model chosen in the composer travel with the next `threads.send`, where the server applies them directly.
 * Cost: one operation per call; one parse per revision; one open stream per space and one per followed turn.
 */
import { spaces } from "../shell/spaces.svelte";
import type { ThreadEvents, ThreadsClient } from "../apps/threads/client";
import type { Answer, Change, Message, Permission, ProviderOption, Request, RunInfo, RunsOn, Step, ThreadState, ThreadSummary } from "../apps/threads/model";
import { ServerClient, ServerError } from "./client";
import { ServerRequests } from "./requests";
import { SpaceFeed, type FeedEntry } from "./space-feed";
import { parseMessages, providerPlugin, readinessNotice, reasonNotice, stateOfTurn, summaryOf, updatedAtOf, wireMode, type WireReadiness, type WireReason, type WireRow, type WireThread, type WireJob } from "./thread-file";

/** Development tracing (`localStorage["cal.debug.threads"] = "1"`): every journal entry, read and stream event with a timestamp, to the console. Off by default; a read of one storage key at start-up. */
const TRACE = (() => { try { return localStorage.getItem("cal.debug.threads") === "1"; } catch { return false; } })();
const t0 = Date.now();
const trace = (...a: unknown[]) => { if (TRACE) console.debug(`[threads +${Date.now() - t0}ms]`, ...a); };

const PROVIDERS: ProviderOption[] = [{ id: "claude", name: "Claude Code", models: [{ id: "default", name: "Default" }] }];

interface Open { thread: WireThread; messages: Message[]; summary: ThreadSummary }
/** The composer's choices for the next send, kept per thread on this page. */
type Choices = Partial<Pick<ThreadSummary, "provider" | "model" | "permission" | "runsOn">>;

export class ServerThreads implements ThreadsClient {
  private choices = new Map<string, Choices>();
  /** The turn state each thread is in, from the list, the thread and the journal (a single source for the sidebar and the open thread). */
  private turns = new Map<string, { state: ThreadState; job?: string }>();
  private opened = new Map<string, Open>();
  private watchers = new Set<(t: ThreadSummary[]) => void>();
  private lastList: ThreadSummary[] = [];
  private follows = new Map<string, { ev: ThreadEvents }>();
  private streams = new Map<string, AbortController>();
  /** The last event of each followed job (`<epoch>:<seq>`), so a stream that drops resumes where it was. */
  private bookmarks = new Map<string, string>();
  private asks: ServerRequests;
  private feed: SpaceFeed | null = null;
  private timers = new Map<string, number>();

  constructor(private server: ServerClient, private space: () => string = () => spaces.current.id) { this.asks = new ServerRequests(server, space); }

  /** The shell's space list is the server's, once: until it is, the current space is a fixture id the server has never heard of. */
  private spacesLoaded: Promise<void> | null = null;
  private loadSpaces(): Promise<void> {
    return (this.spacesLoaded ??= spaces.load().then(() => { if (!spaces.list.some((s) => s.id === spaces.currentId)) spaces.switchTo(spaces.list[0]!.id); }).catch(() => { this.spacesLoaded = null; }));
  }

  private known(id: string): Choices { return this.choices.get(id) ?? {}; }
  private choose(id: string, c: Choices) { this.choices.set(id, { ...this.known(id), ...c }); }
  private summary(id: string, space: string, row: { metadata: WireRow["metadata"]; turn_state?: string; turn_reason?: WireReason | null }): ThreadSummary {
    const t = stateOfTurn(row.turn_state);
    const have = this.turns.get(id);
    this.turns.set(id, { state: t, job: have?.state === t ? have.job : undefined });
    return this.withReadiness(summaryOf(id, space, row.metadata, row.turn_state, this.known(id), row.turn_reason));
  }
  /** Runs `fn` once, 120 ms after the last request for `key`: a burst of journal entries is one read. */
  private later(key: string, fn: () => void, ms = 120) {
    clearTimeout(this.timers.get(key));
    this.timers.set(key, window.setTimeout(() => { this.timers.delete(key); fn(); }, ms));
  }

  // ---- readiness: is there a machine, is it up, is the provider signed in on it (agent.readiness; re-read when the account feed says it changed) ----
  private ready: WireReadiness | null = null;
  private readyFeed: AbortController | null = null;
  /** A thread with nothing from its turn to say gets what the machines say, so "no machine", "offline" and "not signed in" show before a message is sent, not after it fails. */
  private withReadiness(s: ThreadSummary): ThreadSummary {
    if (s.notice || s.state !== "idle") return s;
    const n = readinessNotice(this.ready, providerPlugin(s.provider), PROVIDERS.find((p) => p.id === s.provider)?.name ?? "The provider");
    return n ? { ...s, notice: n } : s;
  }
  private async loadReadiness(): Promise<void> {
    await this.loadSpaces();
    try { this.ready = await this.server.operate<WireReadiness>("agent.readiness", { space_id: this.space() }); } catch (e) { if (!(e instanceof ServerError) || e.code !== "not_found") trace("readiness read failed", String(e)); this.ready = null; }
    this.lastList = this.lastList.map((t) => this.withReadiness({ ...t, notice: undefined }));
    for (const [id, o] of this.opened) { o.summary = this.withReadiness({ ...o.summary, notice: undefined }); this.follows.get(id)?.ev.onSummary({ ...o.summary, ...this.known(id) } as ThreadSummary); }
    this.tellList();
  }
  private watchReadiness(): void {
    if (this.readyFeed) return;
    this.readyFeed = new AbortController();
    const ac = this.readyFeed;
    void this.loadReadiness();
    void (async () => { for (let wait = 1000; !ac.signal.aborted; wait = Math.min(wait * 2, 15000)) { try { await this.server.readinessLive(() => void this.loadReadiness(), ac.signal); } catch { /* away: try again */ } if (ac.signal.aborted) return; await new Promise((r) => setTimeout(r, wait)); } })();
  }
  async check(id: string): Promise<void> {
    const s = this.lastList.find((t) => t.id === id) ?? this.opened.get(id)?.summary;
    const m = this.ready?.machines.find((x) => x.connected) ?? this.ready?.machines[0];
    if (!m?.machine_id || !s) throw new Error("there is no machine of yours to check");
    await this.server.operate("agent.check", { space_id: this.space(), machine_id: m.machine_id, provider_id: providerPlugin(s.provider) });
    await this.loadReadiness();
  }

  async providers(): Promise<ProviderOption[]> { return PROVIDERS; }

  async listThreads(): Promise<ThreadSummary[]> {
    await this.loadSpaces();
    const space = this.space();
    const { threads } = await this.server.operate<{ threads: WireRow[] }>("threads.list", { space_id: space, limit: 1000 });
    this.lastList = threads.map((r) => this.summary(r.id, space, r));
    return this.lastList;
  }

  watchThreads(cb: (t: ThreadSummary[]) => void): () => void {
    this.watchers.add(cb);
    void this.ensureFeed();
    this.watchReadiness();
    return () => { this.watchers.delete(cb); this.maybeStopFeed(); };
  }
  private async refreshList(): Promise<void> {
    trace("list read");
    try { const l = await this.listThreads(); trace("list result", l.length, "rows; first", l[0]?.id.slice(0, 8), l[0]?.title); for (const w of this.watchers) w(l); } catch { /* the server is away: the feed resets and reloads when it is back */ }
  }
  private tellList(): void { for (const w of this.watchers) w(this.lastList); }

  // ---- the live journal ----
  private async ensureFeed(): Promise<void> {
    await this.loadSpaces();
    const space = this.space();
    if (this.feed?.space === space) return;
    this.feed?.stop();
    this.feed = new SpaceFeed(this.server, space, (e) => this.onEntry(e), () => this.onReset());
  }
  private maybeStopFeed(): void {
    if (this.watchers.size || this.follows.size) return;
    this.feed?.stop(); this.feed = null;
    this.readyFeed?.abort(); this.readyFeed = null;
  }
  private onReset(): void {
    void this.refreshList();
    for (const id of this.follows.keys()) this.later(`pull:${id}`, () => void this.pull(id, true));
    void this.asks.refresh();
  }
  /** One journal entry about a thread: a turn's new state is applied as it is; anything else says what to re-read. */
  private onEntry(e: FeedEntry): void {
    trace("feed", e.kind, e.item.slice(0, 8), JSON.stringify(e.data ?? {}).slice(0, 80));
    if (!e.kind.startsWith("threads.") || !e.item) return;
    const id = e.item;
    if (e.kind === "threads.turn_state_changed") {
      const state = stateOfTurn(e.data?.state as string | undefined);
      const before = this.turns.get(id)?.state;
      const job = (e.data?.job_id as string | undefined) ?? undefined;
      this.turns.set(id, { state, job });
      // The entry carries the reason too: the notice changes with the state, no read.
      const notice = reasonNotice(e.data?.turn_reason as WireReason | undefined) ?? undefined;
      this.lastList = this.lastList.map((t) => (t.id === id ? { ...t, state, notice } : t));
      this.tellList();
      const o = this.opened.get(id);
      if (o) { o.summary = { ...o.summary, state, notice }; this.follows.get(id)?.ev.onSummary({ ...o.summary, ...this.known(id) } as ThreadSummary); }
      if (state === "running" && job && this.follows.has(id) && !this.streams.has(id)) void this.follow(id, job);
      if (state === "waiting" || before === "waiting") void this.asks.refresh();
      return;
    }
    if (e.kind === "threads.checkpoint" || e.kind === "threads.send") { if (this.follows.has(id)) this.later(`pull:${id}`, () => void this.pull(id)); }
    else if (this.follows.has(id)) this.later(`pull:${id}`, () => void this.pull(id));
    this.later("list", () => void this.refreshList());
  }

  // ---- one thread ----
  async getThread(id: string): Promise<ThreadSummary | undefined> {
    try { return (await this.load(id)).summary; } catch (e) { if (e instanceof ServerError && e.code === "not_found") return undefined; throw e; }
  }

  /** Reads the thread and parses its messages, unless the cached parse is of the same revision. The running job it names, if any, is remembered for `watch`. */
  private async load(id: string, force = false): Promise<Open> {
    const space = this.space();
    const res = await this.server.operate<{ thread: WireThread; job?: WireJob }>("threads.get", { id, space_id: space });
    const { thread } = res;
    const job = res.job ?? thread.job;
    const base = this.summary(thread.id, thread.space_id, thread);
    if (job && !isFinal(job.state)) this.turns.set(id, { state: base.state, job: job.id });
    const have = this.opened.get(id);
    if (have && have.thread.revision === thread.revision && !force) { have.summary = { ...base, messageCount: have.messages.length }; return have; }
    const updatedAt = updatedAtOf(thread.markdown ?? "", base.updatedAt);
    const messages = parseMessages(thread.markdown ?? "", base.updatedAt, updatedAt);
    const open = { thread, messages, summary: { ...base, updatedAt: Math.max(base.updatedAt, updatedAt), messageCount: messages.length } };
    this.opened.set(id, open);
    return open;
  }

  async messages(id: string, from: number, to: number): Promise<Message[]> {
    const open = this.opened.get(id) ?? (await this.load(id));
    return open.messages.slice(from, to);
  }

  private texts = (ms: Message[]) => ms.map((m) => m.blocks.map((b) => (b.type === "text" ? b.text : "")).join(""));
  private seen = new Map<string, string[]>();
  /** Re-reads the thread and tells the view about every message that is new or changed since the last read. */
  private async pull(id: string, force = false): Promise<void> {
    const f = this.follows.get(id);
    if (!f) return;
    let open: Open;
    trace("pull start", id.slice(0, 8), force ? "forced" : "");
    try { open = await this.load(id, force); } catch { return; }
    const before = this.seen.get(id) ?? [];
    const now = this.texts(open.messages);
    trace("pull read", id.slice(0, 8), "rev", open.thread.revision.slice(0, 8), "messages", now.length, "state", open.summary.state);
    open.messages.forEach((m, i) => { if (now[i] !== before[i]) f.ev.onMessage(m, i); });
    this.seen.set(id, now);
    f.ev.onSummary({ ...open.summary, ...this.known(id) } as ThreadSummary);
  }

  watch(id: string, ev: ThreadEvents): () => void {
    this.follows.set(id, { ev });
    this.seen.set(id, this.texts(this.opened.get(id)?.messages ?? []));
    void this.ensureFeed();
    // Opening (or reloading mid-turn): read the thread, and follow the turn it says is running.
    void (async () => {
      try {
        const o = await this.load(id, true);
        const t = this.turns.get(id);
        if (t?.state === "running" && t.job && !this.streams.has(id)) void this.follow(id, t.job);
        await this.pull(id);
        void o;
      } catch { /* the view shows what it has */ }
    })();
    return () => { this.follows.delete(id); this.seen.delete(id); this.streams.get(id)?.abort(); this.streams.delete(id); clearTimeout(this.timers.get(`pull:${id}`)); this.maybeStopFeed(); };
  }

  /** Follows one job's stream: the working state and steps while it runs, then the saved file at its end. Resumes from the last event it saw. */
  private async follow(id: string, job: string): Promise<void> {
    const f = this.follows.get(id);
    if (!f || this.streams.has(id)) return;
    const abort = new AbortController();
    this.streams.set(id, abort);
    f.ev.onStreamStart(job);
    let n = 0, last: { title: string; at: number } | null = null;
    try {
      await this.server.jobEvents(job, (name, data, eid) => {
        trace("job event", name, eid ?? "");
        if (eid) this.bookmarks.set(job, eid);
        if (name === "reset") { void this.pull(id, true); return; }
        if (name !== "chunk") return;
        // The job stream's chunk is `{ seq, data }`; for a provider event `data` is the provider envelope `{ version, seq, event: { type, ... } }` (docs/dev-e2e.md); placement and
        // notice chunks carry a plain `data.type` and are not provider events.
        const e = (data as { data?: { event?: { type?: string; text?: string; summary?: string; failed?: boolean } } })?.data?.event ?? {};
        const ev = this.follows.get(id)?.ev;
        if (e.type === "text_delta" && e.text) ev?.onDelta(e.text);
        else if (e.type === "tool_started") last = { title: e.summary ?? "Tool", at: Date.now() };
        else if (e.type === "tool_finished" && last) {
          const step: Step = { id: `${job}-${n++}`, kind: "run", title: last.title, status: e.failed ? "failed" : "ok", ms: Date.now() - last.at };
          ev?.onStep(step);
          last = null;
        }
      }, { signal: abort.signal, after: this.bookmarks.get(job) });
    } catch { if (abort.signal.aborted) return; }
    if (abort.signal.aborted) return;
    trace("job stream ended", job.slice(0, 8));
    // The turn ended (or the stream did): the saved file has the answer, and the journal has said what state it is in.
    this.streams.delete(id);
    this.follows.get(id)?.ev.onStreamEnd?.();
    await this.pull(id, true);
  }

  async send(id: string, text: string, requestId: string): Promise<void> {
    const k = this.known(id);
    const input: Record<string, unknown> = { id, space_id: this.space(), message: text };
    // The composer's choices apply directly to this send (the server saves them).
    if (k.permission) input.mode = wireMode(k.permission);
    if (k.provider) input.provider = providerPlugin(k.provider);
    if (k.model && k.model !== "default") input.model = k.model;
    const res = await this.server.operate<{ thread: WireThread; job?: WireJob }>("threads.send", input, requestId);
    this.opened.delete(id);
    if (res.job) {
      this.turns.set(id, { state: "running", job: res.job.id });
      this.announce(id);
      void this.follow(id, res.job.id);
    }
    await this.pull(id);
  }

  async stop(id: string): Promise<void> {
    // The stream goes first: a frame that arrives while the server cancels must not bring the working state back.
    this.streams.get(id)?.abort();
    this.streams.delete(id);
    await this.server.operate("threads.stop", { id, space_id: this.space() });
    this.turns.set(id, { state: "idle" });
    this.follows.get(id)?.ev.onStreamEnd?.();
    this.announce(id);
    await this.pull(id, true);
  }

  /** Tries the last turn again, or continues an interrupted one: `agent.resume` on the thread's last job (`retry_failed` for a failure). */
  async resume(id: string): Promise<void> {
    const t = this.turns.get(id);
    if (!t?.job) throw new Error("this page does not know which turn to resume; open the thread again");
    await this.server.operate("agent.resume", { space_id: this.space(), thread_id: id, job_id: t.job, retry_failed: t.state === "failed" });
  }

  async createThread(title: string, o: { provider?: ThreadSummary["provider"]; parentId?: string } = {}): Promise<ThreadSummary> {
    const input: Record<string, unknown> = { space_id: this.space(), title };
    if (o.provider) input.provider = providerPlugin(o.provider);
    if (o.parentId) input.parent_thread = o.parentId;
    const { thread } = await this.server.operate<{ thread: WireThread }>("threads.create", input);
    return summaryOf(thread.id, thread.space_id, thread.metadata, thread.turn_state);
  }
  async deleteThread(id: string): Promise<void> {
    await this.server.operate("threads.delete", { id, space_id: this.space() });
    this.opened.delete(id); this.choices.delete(id); this.turns.delete(id);
  }
  async rename(id: string, title: string): Promise<void> {
    const t = title.trim();
    if (!t) return;
    await this.server.operate("threads.rename", { id, space_id: this.space(), title: t });
    // The journal tells the list; this makes the title change at once.
    this.lastList = this.lastList.map((x) => (x.id === id ? { ...x, title: t } : x));
    this.tellList();
    const o = this.opened.get(id);
    if (o) { o.summary = { ...o.summary, title: t }; this.follows.get(id)?.ev.onSummary(o.summary); }
  }
  async archive(id: string): Promise<void> {
    trace("archive op start", id.slice(0, 8));
    await this.server.operate("threads.archive", { id, space_id: this.space() });
    trace("archive op done", id.slice(0, 8));
    this.opened.delete(id);
    this.lastList = this.lastList.filter((t) => t.id !== id);
    this.tellList();
  }
  async unarchive(id: string): Promise<boolean> {
    try { await this.server.operate("threads.unarchive", { id, space_id: this.space() }); } catch (e) { if (e instanceof ServerError) return false; throw e; }
    void this.refreshList();
    return true;
  }

  // The composer's choices travel with the next send; the server has no operation for where a thread runs.
  async setProvider(id: string, provider: ThreadSummary["provider"], model: string): Promise<void> { this.choose(id, { provider, model }); this.announce(id); }
  async setPermission(id: string, permission: Permission): Promise<void> { this.choose(id, { permission }); this.announce(id); }
  async machines(): Promise<string[]> { await this.loadReadiness(); return [...(this.ready?.machines.map((m) => m.name ?? "Machine") ?? []), ...(this.ready?.pool.length ? ["Pool"] : [])]; }
  async setRunsOn(id: string, runsOn: RunsOn): Promise<void> { this.choose(id, { runsOn }); this.announce(id); }
  /** Tells the open thread and the list about a change made here, without asking the server. */
  private announce(id: string): void {
    const state = this.turns.get(id)?.state;
    const o = this.opened.get(id);
    if (o) { o.summary = { ...o.summary, ...(state ? { state } : {}) } as ThreadSummary; this.follows.get(id)?.ev.onSummary({ ...o.summary, ...this.known(id) } as ThreadSummary); }
    this.lastList = this.lastList.map((t) => (t.id === id ? ({ ...t, ...this.known(id), ...(state ? { state } : {}) } as ThreadSummary) : t));
    this.tellList();
  }

  // Requests (questions, approvals, secrets) have their own operations and their own small client.
  requests = () => this.asks.requests();
  watchRequests = (cb: (r: Request[]) => void) => this.asks.watchRequests(cb);
  answer = (id: string, a: Answer) => this.asks.answer(id, a);
  async changes(): Promise<Change[]> { return []; }
  async run(id: string): Promise<RunInfo> {
    const s = (await this.getThread(id)) ?? this.lastList.find((t) => t.id === id);
    return { permission: s?.permission ?? "", runsOn: "", workspace: "", branch: "", provider: s?.provider ?? "", model: s?.model ?? "", tokens: "", state: s?.state ?? "" };
  }
}

/** A job state that ends a turn. */
const isFinal = (s: string) => ["done", "failed", "cancelled", "outcome_unknown"].includes(s);
