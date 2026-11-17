/**
 * The Threads app's client on the real server: `ThreadsClient` (apps/threads/client.ts) over the orchestrator's operations (`threads.list/get/create/send/stop/archive/delete`,
 * docs/threads.md) and the job event stream (`GET /api/v1/jobs/<id>/events`, docs/agent-turns.md). Selected with `?server=1` (see `store.svelte.ts`); the fixtures stay the default.
 *
 * What it does and does not have (each gap names what the server lacks, so the UI can say so instead of pretending):
 *   - Messages come from the thread's Markdown (`threads.get`), parsed once per revision and read in ranges from that parse; the 2 MiB file bound is the bound.
 *   - An open thread follows its job's stream for steps and the working state, and re-reads the file every 3 s while open (agent text is appended to the file at checkpoints, so a reload mid-turn still
 *     shows it). The thread list is re-read every 5 s while watched, and after every change made here. (A journal subscription replaces both polls when the server offers a per-kind feed.)
 *   - The list has no per-thread job state: a turn started elsewhere shows as idle until it ends and the file changes. Running state is known for turns started here.
 *   - No operation renames or un-archives a thread, offers machines, or records where a thread runs, and requests (questions, approvals) are not in the server yet: those calls say so (`unsupported`)
 *     or return empty, and the permission mode, provider and model chosen in the composer are sent with the next `threads.send` (`mode`, `provider`, `model`), where the server applies them directly.
 * Cost: one operation per call; one parse per revision; two timers while a thread is open.
 */
import { spaces } from "../shell/spaces.svelte";
import type { ThreadEvents, ThreadsClient } from "../apps/threads/client";
import type { Answer, Change, Message, Permission, ProviderOption, Request, RunInfo, RunsOn, Step, ThreadState, ThreadSummary } from "../apps/threads/model";
import { ServerClient, ServerError } from "./client";
import { parseMessages, providerPlugin, stateOfJob, summaryOf, updatedAtOf, wireMode, type WireJob, type WireRow, type WireThread } from "./thread-file";

/** What the server cannot do yet, as a failure the UI can show. */
export class Unsupported extends Error {
  constructor(what: string) { super(`${what} is not available on this server yet`); }
}

const PROVIDERS: ProviderOption[] = [{ id: "claude", name: "Claude Code", models: [{ id: "default", name: "Default" }] }];
const LIST_EVERY = 5000, OPEN_EVERY = 3000;

interface Open { thread: WireThread; messages: Message[]; summary: ThreadSummary }

export class ServerThreads implements ThreadsClient {
  /** What is known about threads beyond what the list says: state of turns started here, and the composer's choices for the next send. */
  private local = new Map<string, Partial<ThreadSummary> & { job?: string }>();
  private opened = new Map<string, Open>();
  private watchers = new Set<(t: ThreadSummary[]) => void>();
  private listTimer = 0;
  private lastList: ThreadSummary[] = [];

  constructor(private server: ServerClient, private space: () => string = () => spaces.current.id) {}

  private known(id: string) { return this.local.get(id) ?? {}; }
  private patch(id: string, p: Partial<ThreadSummary> & { job?: string }) { this.local.set(id, { ...this.known(id), ...p }); }

  async providers(): Promise<ProviderOption[]> { return PROVIDERS; }

  async listThreads(): Promise<ThreadSummary[]> {
    const space = this.space();
    const { threads } = await this.server.operate<{ threads: WireRow[] }>("threads.list", { space_id: space, limit: 1000 });
    this.lastList = threads.map((r) => summaryOf(r.id, space, r.metadata, this.known(r.id)));
    return this.lastList;
  }

  watchThreads(cb: (t: ThreadSummary[]) => void): () => void {
    this.watchers.add(cb);
    if (!this.listTimer) this.listTimer = window.setInterval(() => { if (!document.hidden) void this.refreshList(); }, LIST_EVERY);
    return () => { this.watchers.delete(cb); if (!this.watchers.size) { clearInterval(this.listTimer); this.listTimer = 0; } };
  }
  private async refreshList(): Promise<void> {
    try { const l = await this.listThreads(); for (const w of this.watchers) w(l); } catch { /* offline: the next tick tries again */ }
  }

  async getThread(id: string): Promise<ThreadSummary | undefined> {
    try { return (await this.load(id)).summary; } catch (e) { if (e instanceof ServerError && e.code === "not_found") return undefined; throw e; }
  }

  /** Reads the thread and parses its messages, unless the cached parse is of the same revision. */
  private async load(id: string, force = false): Promise<Open> {
    const space = this.space();
    const { thread } = await this.server.operate<{ thread: WireThread }>("threads.get", { id, space_id: space });
    const have = this.opened.get(id);
    if (have && have.thread.revision === thread.revision && !force) return have;
    const base = summaryOf(id, thread.space_id, thread.metadata, this.known(id));
    const updatedAt = updatedAtOf(thread.markdown ?? "", base.updatedAt);
    const messages = parseMessages(thread.markdown ?? "", base.updatedAt, updatedAt);
    const open = { thread, messages, summary: { ...base, updatedAt, messageCount: messages.length } };
    this.opened.set(id, open);
    return open;
  }

  async messages(id: string, from: number, to: number): Promise<Message[]> {
    const open = this.opened.get(id) ?? (await this.load(id));
    return open.messages.slice(from, to);
  }

  watch(id: string, ev: ThreadEvents): () => void {
    let live = true;
    const texts = (ms: Message[]) => ms.map((m) => m.blocks.map((b) => (b.type === "text" ? b.text : "")).join(""));
    let seen = texts(this.opened.get(id)?.messages ?? []);
    /** Re-reads the file and tells the view about every message that is new or changed since the last read. */
    const pull = async () => {
      let open: Open;
      try { open = await this.load(id); } catch { return; }
      const now = texts(open.messages);
      open.messages.forEach((m, i) => { if (now[i] !== seen[i]) ev.onMessage(m, i); });
      seen = now;
      ev.onSummary(open.summary);
    };
    const timer = window.setInterval(() => { if (live && !document.hidden) void pull(); }, OPEN_EVERY);
    this.follows.set(id, { ev, pull });
    // A turn that was started here and is still running: follow its stream.
    const job = this.known(id).job;
    if (job && this.known(id).state === "running") void this.follow(id, job);
    return () => { live = false; clearInterval(timer); this.follows.delete(id); this.streams.get(id)?.abort(); };
  }
  private follows = new Map<string, { ev: ThreadEvents; pull: () => Promise<void> }>();
  private streams = new Map<string, AbortController>();

  /** Follows one job's stream: the working state and steps while it runs, then the saved file at its end. */
  private async follow(id: string, job: string): Promise<void> {
    const f = this.follows.get(id);
    this.streams.get(id)?.abort();
    const abort = new AbortController();
    this.streams.set(id, abort);
    f?.ev.onStreamStart(job);
    const started = new Map<number, { title: string; at: number }>();
    let n = 0, last: { title: string; at: number } | null = null;
    try {
      await this.server.jobEvents(job, (name, data) => {
        if (name === "state") { const j = (data as { job?: WireJob })?.job; if (j) this.setState(id, stateOfJob(j.state)); return; }
        if (name !== "chunk") return;
        const e = (data as { data?: { type?: string; text?: string; summary?: string; failed?: boolean } })?.data ?? {};
        const ev = this.follows.get(id)?.ev;
        if (e.type === "text_delta" && e.text) ev?.onDelta(e.text);
        else if (e.type === "tool_started") { last = { title: e.summary ?? "Tool", at: Date.now() }; started.set(n++, last); }
        else if (e.type === "tool_finished" && last) {
          const step: Step = { id: `${job}-${n}`, kind: "run", title: last.title, status: e.failed ? "failed" : "ok", ms: Date.now() - last.at };
          ev?.onStep(step);
          last = null;
        }
      }, { signal: abort.signal });
    } catch (e) { if (abort.signal.aborted) return; }
    // The turn ended (or the stream did): the saved file has the answer.
    this.patch(id, { state: "idle" });
    const open = this.follows.get(id);
    open?.ev.onStreamEnd?.();
    await open?.pull();
    void this.refreshList();
  }
  private setState(id: string, state: ThreadState) {
    if (this.known(id).state === state) return;
    this.patch(id, { state });
    const o = this.opened.get(id);
    if (o) this.follows.get(id)?.ev.onSummary({ ...o.summary, state });
    void this.refreshList();
  }

  async send(id: string, text: string, requestId: string): Promise<void> {
    const k = this.known(id);
    const input: Record<string, unknown> = { id, space_id: this.space(), message: text };
    // The composer's choices apply directly to this send (the server saves them).
    if (k.permission) input.mode = wireMode(k.permission as Permission);
    if (k.provider) input.provider = providerPlugin(k.provider);
    if (k.model && k.model !== "default") input.model = k.model;
    const res = await this.server.operate<{ thread: WireThread; job?: WireJob }>("threads.send", input, requestId);
    this.opened.delete(id);
    if (res.job) { this.patch(id, { state: "running", job: res.job.id }); void this.follow(id, res.job.id); }
    await this.follows.get(id)?.pull();
    void this.refreshList();
  }

  async stop(id: string): Promise<void> {
    await this.server.operate("threads.stop", { id, space_id: this.space() });
    this.streams.get(id)?.abort();
    this.patch(id, { state: "idle" });
    this.follows.get(id)?.ev.onStreamEnd?.();
    await this.follows.get(id)?.pull();
    void this.refreshList();
  }

  async createThread(title: string, o: { provider?: ThreadSummary["provider"]; parentId?: string } = {}): Promise<ThreadSummary> {
    const input: Record<string, unknown> = { space_id: this.space(), title };
    if (o.provider) input.provider = providerPlugin(o.provider);
    if (o.parentId) input.parent_thread = o.parentId;
    const { thread } = await this.server.operate<{ thread: WireThread }>("threads.create", input);
    void this.refreshList();
    return summaryOf(thread.id, thread.space_id, thread.metadata);
  }
  async deleteThread(id: string): Promise<void> {
    await this.server.operate("threads.delete", { id, space_id: this.space() });
    this.opened.delete(id); this.local.delete(id);
    void this.refreshList();
  }
  async archive(id: string): Promise<void> {
    await this.server.operate("threads.archive", { id, space_id: this.space() });
    this.opened.delete(id);
    void this.refreshList();
  }
  async unarchive(): Promise<boolean> { return false; }
  async rename(): Promise<void> { throw new Unsupported("Renaming a thread"); }

  // The composer's choices travel with the next send.
  async setProvider(id: string, provider: ThreadSummary["provider"], model: string): Promise<void> { this.patch(id, { provider, model }); this.announce(id); }
  async setPermission(id: string, permission: Permission): Promise<void> { this.patch(id, { permission }); this.announce(id); }
  /** Tells the open thread and the list about a choice made here, without asking the server. */
  private announce(id: string): void {
    const o = this.opened.get(id);
    if (o) { o.summary = { ...o.summary, ...this.known(id) } as ThreadSummary; this.follows.get(id)?.ev.onSummary(o.summary); }
    this.lastList = this.lastList.map((t) => (t.id === id ? ({ ...t, ...this.known(id) } as ThreadSummary) : t));
    for (const w of this.watchers) w(this.lastList);
  }
  async machines(): Promise<string[]> { return []; }
  async setRunsOn(id: string, runsOn: RunsOn): Promise<void> { this.patch(id, { runsOn }); this.announce(id); }

  async requests(): Promise<Request[]> { return []; }
  watchRequests(): () => void { return () => {}; }
  async answer(_id: string, _a: Answer): Promise<void> { throw new Unsupported("Answering a request"); }
  async changes(): Promise<Change[]> { return []; }
  async run(id: string): Promise<RunInfo> {
    const s = (await this.getThread(id)) ?? this.lastList.find((t) => t.id === id);
    return { permission: s?.permission ?? "", runsOn: "", workspace: "", branch: "", provider: s?.provider ?? "", model: s?.model ?? "", tokens: "", state: s?.state ?? "" };
  }
}
