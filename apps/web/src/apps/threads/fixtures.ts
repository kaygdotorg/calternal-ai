/**
 * Fixture data and `FixtureClient`, the in-memory `ThreadsClient` the app runs on until the real operations exist. Everything
 * here is fabricated and deterministic apart from the secret request's expiry (a fixed 15 minutes from page load) and the streaming
 * timers. Fixture-only; none of it ships with the real client.
 */
import { FIXTURE_SPACES } from "../../shell/spaces.svelte";
import type { ThreadEvents, ThreadsClient } from "./client";
import { messageAt } from "./generate";
import { permissionLabel, runsOnLabel } from "./permissions";
import type { Answer, Change, Message, Permission, Step, ProviderOption, Request, RunInfo, RunsOn, ThreadState, ThreadSummary } from "./model";

const BASE = Date.UTC(2026, 10, 17, 6, 0, 0);
const MIN = 60_000;

/** The id of the 10,000 message thread used by the performance script. */
export const LONG_THREAD = "t-long";

const PROVIDERS: ProviderOption[] = [
  { id: "claude", name: "Claude Code", models: [{ id: "opus-5-5", name: "Opus 5.5" }, { id: "sonnet-5-5", name: "Sonnet 5.5" }, { id: "haiku-5-5", name: "Haiku 5.5" }] },
  { id: "codex", name: "Codex", models: [{ id: "gpt-6-1-sol", name: "GPT-6.1 Sol" }, { id: "gpt-6-luna", name: "GPT-6 Luna" }] },
];

function thread(id: string, title: string, state: ThreadState, workspace: string, ago: number, count: number, parentId: string | null = null, provider: "claude" | "codex" = "claude", model = "sonnet-5-5"): ThreadSummary {
  return { id, spaceId: workspace === "infraternal" ? FIXTURE_SPACES[1]!.id : FIXTURE_SPACES[0]!.id, title, parentId, state, workspace, updatedAt: BASE + 5 * 3600_000 - ago * MIN, provider, model, permission: "approve", runsOn: { machine: "calternal-dev-02-std", mode: "sandboxed" }, messageCount: count };
}

function seedThreads(): ThreadSummary[] {
  return [
    thread("t-run", "Split store.rs by concern", "running", "calternal-ai", 1, 14),
    thread("t-run-c1", "Review the storage change", "running", "calternal-ai", 2, 6, "t-run"),
    thread("t-run-c2", "Update the storage docs", "done", "calternal-ai", 9, 8, "t-run", "codex", "gpt-6-luna"),
    thread("t-wait", "Open the pull request for invites", "waiting", "calternal-ai", 4, 10),
    thread("t-secret", "Publish docs to the Forgejo wiki", "waiting", "calternal-ai", 12, 6),
    thread("t-fail", "Conformance run on dev-02", "failed", "calternal-ai", 40, 12),
    thread(LONG_THREAD, "Journal replay is slow after restart", "idle", "calternal-ai", 95, 10_000, null, "claude", "opus-5-5"),
    { ...thread("t-idle", "Expired invite test", "idle", "calternal-ai", 180, 8, null, "codex", "gpt-6-1-sol"), permission: "least" as const },
    thread("t-mesh-plan", "Plan the mesh rollout", "done", "infraternal", 400, 9),
    { ...thread("t-mesh", "Mesh idle-peer RSS guard", "idle", "infraternal", 320, 12), permission: "full" as const, runsOn: { machine: "kayg-laptop", mode: "direct" as const } },
  ];
}

const QUESTION_FORM = `$branch="lane/invites"
$how="Squash"
$tests=true
Form([Input($branch,"Branch name"), Choice($how,["Rebase","Merge","Squash"],"How should I land it?"), Toggle($tests,"Run the full suite first")], "Before I open the pull request")`;

function seedRequests(): Request[] {
  return [
    { id: "r-q", kind: "question", threadId: "t-wait", title: "Before I open the pull request", form: QUESTION_FORM },
    { id: "r-a", kind: "approval", threadId: "t-wait", title: "Run a command outside the workspace", action: "Push the branch to the Forgejo remote", command: "git push origin lane/invites" },
    { id: "r-s", kind: "secret", threadId: "t-secret", name: "FORGEJO_TOKEN", label: "Forgejo access token", why: "To publish pages to the wiki. The agent gets a handle, never the value.", scope: "personal", required: true, expiresAt: Date.now() + 15 * MIN },
  ];
}

const STREAM_TEXT =
  "I split `store.rs` along the three seams it already had: the journal, the receipts and the index.\n\n" +
  "- `journal.rs` keeps the append path and the tail reader\n- `receipts.rs` owns the request-ID table\n- `index.rs` rebuilds from the journal and nothing else\n\n" +
  "The public API did not change, so no caller moved. Every doc comment went with its item, and `scripts/check-docs.sh` is clean. " +
  "Next I am running the store tests on the dev VM, then I will ask the reviewer thread to read the diff before anything is merged.";

/** The in-memory `ThreadsClient`. */
export class FixtureClient implements ThreadsClient {
  private threadList = seedThreads();
  private requestList = seedRequests();
  private extra = new Map<string, Message[]>();
  private threadWatchers = new Set<(t: ThreadSummary[]) => void>();
  private requestWatchers = new Set<(r: Request[]) => void>();
  private open = new Map<string, { events: Set<ThreadEvents>; timer?: ReturnType<typeof setInterval>; sent: number; text: string }>();
  /** Milliseconds between streamed chunks; the performance script lowers it. */
  streamInterval = 40;

  async providers(): Promise<ProviderOption[]> {
    return PROVIDERS;
  }
  async listThreads(): Promise<ThreadSummary[]> {
    return this.threadList;
  }
  watchThreads(cb: (t: ThreadSummary[]) => void): () => void {
    this.threadWatchers.add(cb);
    return () => this.threadWatchers.delete(cb);
  }
  private summary(id: string): ThreadSummary {
    const t = this.threadList.find((x) => x.id === id);
    if (!t) throw new Error(`no such thread ${id}`);
    return t;
  }
  private patch(id: string, p: Partial<ThreadSummary>): void {
    this.threadList = this.threadList.map((t) => (t.id === id ? { ...t, ...p } : t));
    for (const w of this.threadWatchers) w(this.threadList);
    const o = this.open.get(id);
    if (o) for (const e of o.events) e.onSummary(this.summary(id));
  }

  async getThread(threadId: string): Promise<ThreadSummary | undefined> {
    return this.threadList.find((t) => t.id === threadId);
  }
  async createThread(title: string, o: { provider?: ThreadSummary["provider"]; parentId?: string } = {}): Promise<ThreadSummary> {
    const t: ThreadSummary = { id: `t-new-${this.threadList.length}`, spaceId: FIXTURE_SPACES[0]!.id, title, parentId: o.parentId ?? null, state: "idle", workspace: "calternal-ai", updatedAt: Date.now(), provider: o.provider ?? "claude", model: "sonnet-5-5", permission: "approve", runsOn: { machine: "calternal-dev-02-std", mode: "sandboxed" }, messageCount: 0 };
    this.threadList = [t, ...this.threadList];
    for (const w of this.threadWatchers) w(this.threadList);
    return t;
  }
  async deleteThread(threadId: string): Promise<void> {
    this.threadList = this.threadList.filter((t) => t.id !== threadId);
    for (const w of this.threadWatchers) w(this.threadList);
  }

  async messages(threadId: string, from: number, to: number): Promise<Message[]> {
    const t = this.summary(threadId);
    const seed = [...threadId].reduce((a, c) => a + c.charCodeAt(0), 0);
    const base = this.extra.get(threadId) ?? [];
    const generated = t.messageCount - base.length;
    const out: Message[] = [];
    for (let i = from; i < Math.min(to, t.messageCount); i++) out.push(i < generated ? messageAt(threadId, seed, i, BASE) : base[i - generated]!);
    return out;
  }

  watch(threadId: string, events: ThreadEvents): () => void {
    let o = this.open.get(threadId);
    if (!o) this.open.set(threadId, (o = { events: new Set(), sent: 0, text: "" }));
    o.events.add(events);
    if (this.summary(threadId).state === "running" && !o.timer) queueMicrotask(() => this.startStream(threadId));
    return () => {
      const cur = this.open.get(threadId);
      if (!cur) return;
      cur.events.delete(events);
      if (!cur.events.size) {
        clearInterval(cur.timer);
        this.open.delete(threadId);
      }
    };
  }

  private startStream(threadId: string): void {
    const o = this.open.get(threadId);
    if (!o || o.timer) return;
    const id = `${threadId}-stream-${this.summary(threadId).messageCount}`;
    o.sent = 0;
    for (const e of o.events) e.onStreamStart(id);
    let stepN = 0;
    o.timer = setInterval(() => {
      // A tool step about every 700 ms of the run, so the working state has something to show.
      if (o.sent % 60 < 5 && o.sent / 60 >= stepN) {
        const s: Step = { id: `${id}-s${stepN}`, kind: (["read", "search", "edit", "run"] as const)[stepN % 4]!, title: ["Read", "Searched", "Edited", "Ran"][stepN % 4]!, detail: ["crates/store/src/lib.rs", "rg -n 'sync_all' crates", "crates/store/src/journal.rs", "cargo test -j6 -p calternal-store"][stepN % 4], status: "ok", ms: 400 + stepN * 150 };
        stepN++;
        this.stepLog.set(id, [...(this.stepLog.get(id) ?? []), s]);
        for (const e of o.events) e.onStep(s);
      }
      const step = 4 + (o.sent % 5);
      const chunk = STREAM_TEXT.slice(o.sent, o.sent + step);
      o.sent += step;
      for (const e of o.events) e.onDelta(chunk);
      if (o.sent >= STREAM_TEXT.length) {
        clearInterval(o.timer);
        o.timer = undefined;
        this.finishStream(threadId, id, STREAM_TEXT.slice(0, o.sent));
      }
    }, this.streamInterval);
  }

  private stepLog = new Map<string, Step[]>();
  private stepsOf(id: string): Step[] {
    return this.stepLog.get(id) ?? [];
  }

  private finishStream(threadId: string, id: string, text: string): void {
    const t = this.summary(threadId);
    const msg: Message = { id, role: "agent", at: Date.now(), blocks: [{ type: "steps", steps: this.stepsOf(id), ms: 12_000 }, { type: "text", text }] };
    this.extra.set(threadId, [...(this.extra.get(threadId) ?? []), msg]);
    this.patch(threadId, { messageCount: t.messageCount + 1, state: "idle", updatedAt: Date.now() });
    const o = this.open.get(threadId);
    if (o) for (const e of o.events) e.onMessage(msg, t.messageCount);
  }

  async send(threadId: string, text: string, _requestId: string): Promise<void> {
    const t = this.summary(threadId);
    const msg: Message = { id: `${threadId}-u${t.messageCount}`, role: "user", at: Date.now(), blocks: [{ type: "text", text }] };
    this.extra.set(threadId, [...(this.extra.get(threadId) ?? []), msg]);
    this.patch(threadId, { messageCount: t.messageCount + 1, state: "running", updatedAt: Date.now() });
    const o = this.open.get(threadId);
    if (o) for (const e of o.events) e.onMessage(msg, t.messageCount);
    if (o && !o.timer) setTimeout(() => this.startStream(threadId), 300);
  }
  async stop(threadId: string): Promise<void> {
    const o = this.open.get(threadId);
    if (o?.timer) {
      clearInterval(o.timer);
      o.timer = undefined;
      this.finishStream(threadId, `${threadId}-stopped-${Date.now()}`, STREAM_TEXT.slice(0, o.sent));
    } else this.patch(threadId, { state: "idle" });
  }
  async rename(threadId: string, title: string): Promise<void> {
    if (title.trim()) this.patch(threadId, { title: title.trim() });
  }
  async archive(threadId: string): Promise<void> {
    const gone = new Set([threadId, ...this.threadList.filter((t) => t.parentId === threadId && t.state === "done").map((t) => t.id)]);
    this.archived.set(threadId, { threads: this.threadList.filter((t) => gone.has(t.id)), requests: this.requestList.filter((r) => gone.has(r.threadId)) });
    this.threadList = this.threadList.filter((t) => !gone.has(t.id));
    this.requestList = this.requestList.filter((r) => !gone.has(r.threadId));
    for (const w of this.threadWatchers) w(this.threadList);
    for (const w of this.requestWatchers) w(this.requestList);
  }
  private archived = new Map<string, { threads: ThreadSummary[]; requests: Request[] }>();
  async unarchive(threadId: string): Promise<boolean> {
    const a = this.archived.get(threadId);
    if (!a) return false;
    this.archived.delete(threadId);
    this.threadList = [...this.threadList, ...a.threads];
    this.requestList = [...this.requestList, ...a.requests];
    for (const w of this.threadWatchers) w(this.threadList);
    for (const w of this.requestWatchers) w(this.requestList);
    return true;
  }
  async setPermission(threadId: string, permission: Permission): Promise<void> {
    this.patch(threadId, { permission });
  }
  async machines(): Promise<string[]> {
    return ["calternal-dev-02-std", "calternal-dev-01-compute02", "kayg-laptop"];
  }
  async setRunsOn(threadId: string, runsOn: RunsOn): Promise<void> {
    this.patch(threadId, { runsOn });
  }
  async setProvider(threadId: string, provider: ThreadSummary["provider"], model: string): Promise<void> {
    this.patch(threadId, { provider, model });
  }

  async requests(): Promise<Request[]> {
    return this.requestList;
  }
  watchRequests(cb: (r: Request[]) => void): () => void {
    this.requestWatchers.add(cb);
    return () => this.requestWatchers.delete(cb);
  }
  async answer(requestId: string, _answer: Answer): Promise<void> {
    const r = this.requestList.find((x) => x.id === requestId);
    this.requestList = this.requestList.filter((x) => x.id !== requestId);
    for (const w of this.requestWatchers) w(this.requestList);
    if (r && !this.requestList.some((x) => x.threadId === r.threadId)) this.patch(r.threadId, { state: "running" });
  }

  async changes(_threadId: string): Promise<Change[]> {
    return [
      { path: "crates/store/src/journal.rs", status: "added", added: 212, removed: 0 },
      { path: "crates/store/src/receipts.rs", status: "added", added: 148, removed: 0 },
      { path: "crates/store/src/index.rs", status: "added", added: 96, removed: 0 },
      { path: "crates/store/src/lib.rs", status: "modified", added: 18, removed: 431 },
      { path: "docs/storage-layout.md", status: "modified", added: 7, removed: 2 },
    ];
  }
  async run(threadId: string): Promise<RunInfo> {
    const t = this.summary(threadId);
    return { permission: permissionLabel(t.permission), runsOn: runsOnLabel(t.runsOn), workspace: t.workspace, branch: `lane/${t.id.replace(/^t-/, "")}`, provider: t.provider === "claude" ? "Claude Code" : "Codex", model: t.model, tokens: "184k in, 22k out", state: t.state };
  }
}
