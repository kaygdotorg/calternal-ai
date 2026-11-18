// The server adapter against a recorded-response double: the response shapes are the ones `crates/server/src/threads` returns (docs/threads.md), the job events are
// the SSE frames `job_api.rs` writes (provider events nested as chunk.data.event, docs/dev-e2e.md), the journal entries are the kinds docs/threads.md lists. No network.
import { describe, expect, test } from "vitest";
import { ServerClient } from "./client";
import { ServerThreads } from "./threads-client";
import { lifetimesOf } from "./requests";
import { applyAppend, bodyOf } from "./thread-delta";
import { blocksOf, parseMessages, readinessNotice, reasonNotice, stateOfTurn, summaryOf, type WireMachine, type WireReadiness } from "./thread-file";
import type { ThreadEvents } from "../apps/threads/client";

const SPACE = "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11";
const T1 = "019a8c00-0000-7000-8000-000000000001";
const T2 = "019a8c00-0000-7000-8000-000000000002";
const JOB = "job-0000000000000001";
const M1 = "019a8c00-0000-7000-8000-0000000000a1";
const M2 = "019a8c00-0000-7000-8000-0000000000a2";
const md = (extra = "") => `---\ntitle: "Plan"\nprovider: "calternal-claude-code"\nmode: "approve_for_me"\ncreated-at: "2026-11-17T12:00:00.000Z"\nupdated-at: "2026-11-17T12:05:00.000Z"\n---\n\n<!-- message:${M1} role:person time:2026-11-17T12:01:00.000Z actor:x -->\nInspect this workspace.\n<!-- /message -->\n${extra}`;
const agent = `\n<!-- message:${M2} role:agent time:2026-11-17T12:04:00.000Z turn:${JOB} actor:y -->\nI looked. Nothing wrong.\n<!-- /message -->\n`;
const meta = (o: object = {}) => ({ title: "Plan", provider: "calternal-claude-code", model: null, mode: "approve_for_me", mode_for: "acc", message_count: 1, last_activity_at: "2026-11-17T12:04:00.000Z", created_at: "2026-11-17T12:00:00.000Z", archived: false, deleted: false, ...o });
const thread = (markdown: string, revision = "r1", o: object = {}, turn_state = "idle") => ({ thread: { id: T1, space_id: SPACE, address: `/threads/${T1}`, revision, turn_state, metadata: meta(o), markdown } });
const row = (id: string, turn_state = "idle", o: object = {}) => ({ id, address: `/threads/${id}`, revision: "r", turn_state, metadata: meta(o) });
const enc = new TextEncoder();
const frame = (n: string, d: unknown) => enc.encode(`event: ${n}\ndata: ${JSON.stringify(d)}\n\n`);
const sse = (frames: [string, unknown][], closeAfter = 40) => new Response(new ReadableStream<Uint8Array>({ start(c) { for (const [n, d] of frames) c.enqueue(frame(n, d)); setTimeout(() => c.close(), closeAfter); } }), { headers: { "content-type": "text/event-stream" } });
const ok = (data: unknown) => new Response(JSON.stringify({ request_id: "r", ok: true, data }), { headers: { "content-type": "application/json" } });
const fail = (code: string, message: string) => new Response(JSON.stringify({ request_id: "r", ok: false, error: { code, message } }), { status: 404 });
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
/** Waits for the condition itself (polling every 5 ms, up to 5 s), never for a guessed time: a loaded machine makes it slower, not wrong. Absence is the only thing a fixed wait is used for. */
const until = async (cond: () => boolean, what = "the condition") => { for (let i = 0; i < 1000; i++) { if (cond()) return; await wait(5); } throw new Error(`timed out waiting for ${what}`); };

/** A double of the server: records every operation, answers from `handlers`; the space's journal is a stream the test pushes entries into. */
function double(handlers: Record<string, (input: any, requestId: string) => unknown>) {
  const calls: { operation: string; input: any; requestId: string }[] = [];
  let events: Response | null = null;
  const eventUrls: string[] = [];
  let live: ReadableStreamDefaultController<Uint8Array> | null = null;
  const fetch = (async (url: string, init?: RequestInit) => {
    if (url.includes("/jobs/")) { eventUrls.push(url); return events ?? fail("not_found", "no job"); }
    if (url.includes("/api/v1/live")) return new Response(new ReadableStream<Uint8Array>({ start(c) { live = c; c.enqueue(frame("message", { type: "ready", space_id: SPACE, bookmark: "b0" })); } }), { headers: { "content-type": "text/event-stream" } });
    const b = JSON.parse(init!.body as string);
    calls.push({ operation: b.operation, input: b.input, requestId: b.request_id });
    const h = handlers[b.operation];
    return h ? (() => { const r = h(b.input, b.request_id); return r instanceof Response ? r : ok(r); })() : fail("not_found", "unknown operation");
  }) as never;
  const reverified: number[] = [];
  const client = new ServerThreads(new ServerClient({ token: () => "T", fetch }), () => SPACE, async () => { reverified.push(1); });
  const push = (kind: string, item: string, data?: object) => live!.enqueue(frame("message", { type: "entry", space_id: SPACE, entry: { kind, item, data } }));
  return { client, calls, reverified, eventUrls, hasFeed: () => live !== null, push, setEvents: (r: Response) => (events = r), ops: (name: string) => calls.filter((c) => c.operation === name) };
}

describe("thread_full", () => {
  test("a turn that failed because the thread reached its size limit says so and offers a new thread", async () => {
    const { reasonNotice } = await import("./thread-file");
    expect(reasonNotice({ code: "thread_full" })).toEqual({ kind: "thread-full" });
  });
});

describe("the saved thread's Markdown", () => {
  test("messages are read in order, with roles and ids; unclosed ones run to the next marker", () => {
    const ms = parseMessages(md(agent), 1000, 2000);
    expect(ms.map((m) => [m.id, m.role])).toEqual([[M1, "user"], [M2, "agent"]]);
    expect(ms[0]!.blocks).toEqual([{ type: "text", text: "Inspect this workspace." }]);
    expect(ms[1]!.at).toBe(Date.parse("2026-11-17T12:04:00.000Z"));
    const open = parseMessages(`---\ntitle: "x"\n---\n<!-- message:${M1} role:agent -->\nhalf wri`);
    expect(open).toHaveLength(1);
    expect((open[0]!.blocks[0] as { text: string }).text).toBe("half wri");
  });
  test("message text that looks like a marker inside a body cannot add a role: only real markers split", () => {
    const ms = parseMessages(`---\ntitle: "x"\n---\n<!-- message:${M1} role:person -->\nsay <!-- hello --> ok\n<!-- /message -->\n`);
    expect(ms).toHaveLength(1);
  });
  test("each message has the time its marker records", () => {
    const ms = parseMessages(md(agent), 1000, 2000);
    expect(ms.map((m) => m.at)).toEqual([Date.parse("2026-11-17T12:01:00.000Z"), Date.parse("2026-11-17T12:04:00.000Z")]);
  });
  test("turn labels map to the three states that mean something", () => {
    expect(["queued", "running"].map(stateOfTurn)).toEqual(["running", "running"]);
    expect(["waiting", "interrupted"].map(stateOfTurn)).toEqual(["waiting", "waiting"]);
    expect(stateOfTurn("failed")).toBe("failed");
    expect(["idle", undefined].map(stateOfTurn)).toEqual(["idle", "idle"]);
  });
  test("a summary carries everything the sidebar shows, from the list row alone", () => {
    const s = summaryOf(T1, SPACE, meta({ mode: "full_access", provider: "calternal-codex", parent_thread: T2, model: "sonnet", message_count: 7 }), "running");
    expect([s.provider, s.permission, s.parentId, s.model, s.messageCount, s.state, s.updatedAt]).toEqual(["codex", "full", T2, "sonnet", 7, "running", Date.parse("2026-11-17T12:04:00.000Z")]);
  });
});

describe("turn reasons (turn readiness: turn_reason on the list, the thread and the journal)", () => {
  test("every code that asks something of the person becomes its notice; the rest ask nothing", () => {
    expect(reasonNotice({ code: "no_machine" })).toEqual({ kind: "no-machine" });
    expect(reasonNotice({ code: "waiting_for_capacity", queue_position: 2 })).toEqual({ kind: "waiting-capacity", queuePosition: 2 });
    expect(reasonNotice({ code: "machine_offline", since_ms: 5 })).toMatchObject({ kind: "machine-offline", sinceMs: 5 });
    expect(reasonNotice({ code: "provider_signed_out", provider: "Claude Code", sign_in_command: "claude auth login" })).toEqual({ kind: "provider-signin", provider: "Claude Code", command: "claude auth login" });
    expect(reasonNotice({ code: "retrying", attempt: 3, next_attempt_ms: 9, rule: "provider_backoff" })).toEqual({ kind: "retrying", rule: "provider_backoff", attempt: 3, of: 5, nextAt: 9 });
    expect(reasonNotice({ code: "retrying", rule: "subscription_reset", next_attempt_ms: 9 })).toMatchObject({ rule: "subscription_reset" });
    expect(reasonNotice({ code: "failed", class: "usage_limit", reset_at_ms: 7 })).toEqual({ kind: "failed", failure: "usage_limit", resetAtMs: 7 });
    expect(reasonNotice({ code: "failed", class: "permanent" })).toMatchObject({ failure: "permanent" });
    expect(reasonNotice({ code: "interrupted", resumable: false, why: "unsure" })).toMatchObject({ kind: "interrupted", resumable: false });
    for (const c of ["idle", "queued", "completed", "cancelled", "waiting_on_person"]) expect(reasonNotice({ code: c })).toBeNull();
    expect(reasonNotice(null)).toBeNull();
  });
  test("a list row's reason is the thread's notice; without one, the turn label stands in", () => {
    const withReason = summaryOf(T1, SPACE, meta(), "idle", {}, { code: "no_machine" });
    expect(withReason.notice).toEqual({ kind: "no-machine" });
    expect(summaryOf(T1, SPACE, meta(), "failed").notice).toMatchObject({ kind: "failed", failure: "transient" });
    expect(summaryOf(T1, SPACE, meta(), "idle").notice).toBeUndefined();
  });
  test("a turn-state entry carries the reason: the notice changes with no read", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1)] }) });
    const seen: (string | undefined)[] = [];
    await d.client.listThreads();
    const off = d.client.watchThreads((l) => seen.push(l[0]!.notice?.kind));
    await until(() => d.hasFeed(), "the journal subscription");
    const lists = d.ops("threads.list").length;
    d.push("threads.turn_state_changed", T1, { job_id: JOB, state: "queued", job_state: "admitted", turn_reason: { code: "no_machine" } });
    await until(() => seen.includes("no-machine"), "the notice");
    off();
    expect(d.ops("threads.list")).toHaveLength(lists);
  });
});

const PROV = "calternal-claude-code";
const machine = (o: Partial<WireMachine> = {}, p: Partial<WireMachine["providers"][string]> = {}): WireMachine => ({ machine_id: "m1", name: "dev-02", last_seen_ms: Date.now(), enrolled: true, connected: true, available: true, execution: "sandboxed", providers: { [PROV]: { enabled: true, check: { installed: true, signed_in: true, sign_in_command: "claude auth login" }, ready: true, ...p } }, ...o });
const snap = (machines: WireMachine[], pool: WireMachine[] = []): WireReadiness => ({ machines, pool, check_ttl_ms: 300000 });

describe("readiness (agent.readiness): before a message is sent", () => {
  test("says nothing when some machine is up with the provider signed in", () => {
    expect(readinessNotice(snap([machine()]), PROV, "Claude Code")).toBeNull();
    expect(readinessNotice(snap([machine({ connected: false })], [machine({ machine_id: undefined, name: undefined })]), PROV, "Claude Code")).toBeNull();
  });
  test("no machine at all", () => { expect(readinessNotice(snap([]), PROV, "Claude Code")).toEqual({ kind: "no-machine" }); });
  test("every machine offline: the one seen last is named", () => {
    const n = readinessNotice(snap([machine({ connected: false, name: "old", last_seen_ms: 1 }), machine({ connected: false, name: "newer", last_seen_ms: 9 })]), PROV, "Claude Code");
    expect(n).toMatchObject({ kind: "machine-offline", machine: "newer", sinceMs: 9 });
  });
  test("signed out on the machine that is up: with the provider's own command", () => {
    const n = readinessNotice(snap([machine({}, { ready: false, check: { installed: true, signed_in: false, sign_in_command: "claude auth login" } })]), PROV, "Claude Code");
    expect(n).toEqual({ kind: "provider-signin", machine: "dev-02", provider: "Claude Code", command: "claude auth login" });
  });
  test("a provider not observed yet is not called signed out", () => {
    expect(readinessNotice(snap([machine({}, { ready: false, check: null })]), PROV, "Claude Code")).toBeNull();
  });
  test("the thread carries it until its turn says something else; Check again calls agent.check and re-reads", async () => {
    let signedIn = false;
    const d = double({
      "threads.list": () => ({ threads: [row(T1)] }),
      "agent.readiness": () => snap([machine({}, { ready: signedIn, check: { installed: true, signed_in: signedIn, sign_in_command: "claude auth login" } })]),
      "agent.check": () => ({}),
    });
    d.client.watchThreads(() => {});
    await until(() => d.ops("agent.readiness").length > 0, "the readiness read");
    const l = await d.client.listThreads();
    expect(l[0]!.notice).toMatchObject({ kind: "provider-signin", command: "claude auth login" });
    signedIn = true;
    await d.client.check(T1);
    expect(d.ops("agent.check")[0]!.input).toEqual({ space_id: SPACE, machine_id: "m1", provider_id: PROV });
    expect((await d.client.listThreads())[0]!.notice).toBeUndefined();
  });
  test("the machines offered are the account's own, and the pool when it has one", async () => {
    const d = double({ "agent.readiness": () => snap([machine(), machine({ machine_id: "m2", name: "laptop" })], [machine({ machine_id: undefined, name: undefined })]) });
    expect(await d.client.machines()).toEqual(["dev-02", "laptop", "Pool"]);
  });
});

describe("ServerThreads", () => {
  test("the list is one operation and carries the turn state, provider, model, count and parent", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1, "running", { model: "sonnet", message_count: 3 }), row(T2, "idle", { title: "Child", parent_thread: T1 })] }) });
    const l = await d.client.listThreads();
    expect(l.map((t) => [t.id, t.parentId, t.state, t.model, t.messageCount])).toEqual([[T1, null, "running", "sonnet", 3], [T2, T1, "idle", "default", 1]]);
    expect(d.ops("threads.list")).toHaveLength(1);
    expect(d.ops("threads.get")).toHaveLength(0);
  });
  test("opening reads the file once per revision and ranges come from the parse", async () => {
    let gets = 0;
    const d = double({ "threads.get": () => { gets++; return thread(md(agent)); } });
    const s = await d.client.getThread(T1);
    expect(s?.messageCount).toBe(2);
    expect((await d.client.messages(T1, 1, 5)).map((m) => m.id)).toEqual([M2]);
    expect(gets).toBe(1);
  });
  test("a thread that is not there (or not visible) is undefined, not an error", async () => {
    const d = double({ "threads.get": () => fail("not_found", "item not found") });
    expect(await d.client.getThread(T1)).toBeUndefined();
  });
  test("nothing is polled: with the feed quiet there are no more reads", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1)] }), "threads.get": () => thread(md()) });
    const off = d.client.watchThreads(() => {});
    const offWatch = d.client.watch(T1, { onMessage() {}, onStreamStart() {}, onDelta() {}, onStep() {}, onSummary() {} });
    await until(() => d.hasFeed() && d.ops("threads.get").length >= 1, "the first reads");
    for (let still = 0, last = -1; still < 4; await wait(40)) { still = d.calls.length === last ? still + 1 : 0; last = d.calls.length; } // the opening reads have finished
    const n = d.calls.length;
    await wait(400);
    off(); offWatch();
    expect(d.calls.length).toBe(n);
  });
  test("a turn-state entry changes the state in place, with no read", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1)] }) });
    const seen: string[] = [];
    await d.client.listThreads();
    const off = d.client.watchThreads((l) => seen.push(l[0]!.state));
    await until(() => d.hasFeed(), "the journal subscription");
    const lists = d.ops("threads.list").length;
    d.push("threads.turn_state_changed", T1, { job_id: JOB, state: "running", job_state: "running" });
    await until(() => seen.includes("running"), "the state change");
    off();
    expect(seen).toContain("running");
    expect(d.ops("threads.list")).toHaveLength(lists);
  });
  test("a created, renamed or archived thread re-reads the list once, however many entries arrive", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1)] }) });
    const off = d.client.watchThreads(() => {});
    await until(() => d.hasFeed(), "the journal subscription");
    const lists = d.ops("threads.list").length;
    d.push("threads.rename", T1); d.push("threads.archive", T2); d.push("threads.create", T2);
    await until(() => d.ops("threads.list").length > lists, "the list re-read");
    await wait(300); // absence: no second read follows the burst
    off();
    expect(d.ops("threads.list")).toHaveLength(lists + 1);
  });
  test("a checkpoint of the open thread re-reads that thread and tells the view the changed message", async () => {
    let rev = 1;
    const d = double({ "threads.get": () => (rev === 1 ? thread(md(), "r1") : thread(md(agent), "r2")), "threads.list": () => ({ threads: [row(T1)] }) });
    const got: string[] = [];
    const off = d.client.watch(T1, { onMessage: (m, i) => got.push(`${i} ${m.role}`), onStreamStart() {}, onDelta() {}, onStep() {}, onSummary() {} });
    await until(() => got.includes("0 user") && d.hasFeed(), "the thread's first read and the journal subscription");
    rev = 2;
    d.push("threads.checkpoint", T1);
    await until(() => got.includes("1 agent"), "the changed message");
    off();
    expect(got).toContain("1 agent");
  });
  test("a reloaded page follows the running turn the thread names, and resumes from the last event it saw", async () => {
    const running = { ...thread(md(), "r1", {}, "running"), job: { id: JOB, state: "running" } };
    const d = double({ "threads.get": () => running, "threads.list": () => ({ threads: [row(T1, "running")] }) });
    d.setEvents(sse([["chunk", { seq: 1, data: { version: 1, seq: 1, event: { type: "tool_started", summary: "Read a" } } }]], 5000));
    const started: string[] = [];
    const off = d.client.watch(T1, { onMessage() {}, onStreamStart: (id) => started.push(id), onDelta() {}, onStep() {}, onSummary() {} });
    await until(() => started.length > 0, "the turn being followed");
    off();
    expect(started).toEqual([JOB]);
    expect(d.eventUrls[0]).toContain(JOB);
  });
  test("send carries the composer's mode, provider and model, with the caller's request id, and follows the job: steps, then the saved answer", async () => {
    const offset = new TextEncoder().encode(md().split("\n---\n")[1]!).length;
    const d = double({
      // The first read is the whole thread; the saved answer arrives as a checkpoint addition (`since`), never as a second whole read.
      "threads.get": (i) => (i.since ? { delta: { id: T1, base_revision: "r1", revision: "r2", offset, markdown: agent } } : thread(md(), "r1")),
      "threads.send": () => ({ ...thread(md(), "r1b"), job: { id: JOB, state: "queued" } }),
    });
    d.setEvents(sse([
      ["snapshot", { job: { id: JOB, state: "running" }, epoch: "e" }],
      ["chunk", { seq: 0, data: { type: "placement", note: "not a provider event" } }],
      ["chunk", { seq: 1, data: { version: 1, seq: 1, event: { type: "tool_started", summary: "Read src/lib.rs" } } }],
      ["chunk", { seq: 2, data: { version: 1, seq: 2, event: { type: "tool_finished", failed: false } } }],
      ["chunk", { seq: 3, data: { version: 1, seq: 3, event: { type: "text_delta", text: "I looked." } } }],
    ], 150)); // the stream ends after the file has the answer (the flip below)
    const got: string[] = [];
    const ev: ThreadEvents = { onMessage: (m, i) => got.push(`message ${i} ${m.role}`), onStreamStart: () => got.push("start"), onDelta: (t) => got.push(`delta ${t}`), onStep: (s) => got.push(`step ${s.title} ${s.status}`), onSummary: () => {}, onStreamEnd: () => got.push("end") };
    await d.client.getThread(T1);
    const off = d.client.watch(T1, ev);
    await until(() => d.hasFeed(), "the space feed"); // the header's read is the one the open starts from
    await d.client.setPermission(T1, "full");
    await d.client.setProvider(T1, "claude", "sonnet-5-5");
    await d.client.send(T1, "Inspect this workspace.", "req-1");
    await until(() => got.includes("end"), "the end of the stream");
    d.push("threads.checkpoint", T1, { revision: "r2", append: { base_revision: "r1", offset, start: 0, length: agent.length } });
    await until(() => got.includes("message 1 agent"), "the saved answer");
    expect(d.ops("threads.get").filter((c) => c.input.since)).toHaveLength(1);
    expect(d.ops("threads.get").filter((c) => !c.input.since).length).toBeLessThanOrEqual(3); // the test's own read, the open, and one after the send dropped the cache: none for the checkpoint
    off();
    const send = d.ops("threads.send")[0]!;
    expect(send.requestId).toBe("req-1");
    expect(send.input).toEqual({ id: T1, space_id: SPACE, message: "Inspect this workspace.", mode: "full_access", provider: "calternal-claude-code", model: "sonnet-5-5" });
    expect(got.filter((x) => !x.startsWith("message 0"))).toEqual(["start", "step Read src/lib.rs ok", "delta I looked.", "end", "message 1 agent"]);
  });
  test("stop, archive, rename, unarchive and delete are one operation each, on the thread's own id", async () => {
    const d = double({ "threads.stop": () => thread(md()), "threads.archive": () => thread(md()), "threads.rename": () => thread(md()), "threads.unarchive": () => thread(md()), "threads.delete": () => thread(md()), "threads.get": () => thread(md()) });
    await d.client.stop(T1); await d.client.archive(T1); await d.client.rename(T1, " Better title "); expect(await d.client.unarchive(T1)).toBe(true); await d.client.deleteThread(T1);
    expect(d.calls.filter((c) => /^threads\.(stop|archive|rename|unarchive|delete)$/.test(c.operation)).map((c) => [c.operation, c.input.id, c.input.title])).toEqual([["threads.stop", T1, undefined], ["threads.archive", T1, undefined], ["threads.rename", T1, "Better title"], ["threads.unarchive", T1, undefined], ["threads.delete", T1, undefined]]);
  });
  test("un-archiving something the server refuses resolves false", async () => {
    const d = double({ "threads.unarchive": () => fail("not_found", "item not found") });
    expect(await d.client.unarchive(T1)).toBe(false);
  });
  test("create sends the title, provider and parent", async () => {
    const d = double({ "threads.create": () => thread("") });
    const t = await d.client.createThread("Plan", { provider: "claude", parentId: T2 });
    expect(d.calls.find((c) => c.operation === "threads.create")!.input).toEqual({ space_id: SPACE, title: "Plan", provider: "calternal-claude-code", parent_thread: T2 });
    expect(t.id).toBe(T1);
  });
  test("what the server cannot do says so", async () => {
    const d = double({});
    expect(await d.client.machines()).toEqual([]);
    expect(await d.client.changes()).toEqual([]);
  });
});

describe("requests (docs/requests.md, crates/core/src/requests.rs)", () => {
  const list = { requests: [
    { id: "q1", space_id: SPACE, thread_id: T1, detail: { kind: "question", label: "Before I open the pull request", form: { genui_version: 2, source: "Form([],\"x\")" } } },
    { id: "a1", space_id: SPACE, thread_id: T1, detail: { kind: "approval", app: "calternal-notes", write: true, action: { operation: "threads.delete", input: { id: T2 }, request_id: "r" }, lifetimes: ["once"] } },
    { id: "a2", space_id: SPACE, thread_id: T1, detail: { kind: "approval", app: "calternal-notes", write: false, action: { operation: "notes.get", input: { id: "n" }, request_id: "r" }, lifetimes: ["once", "this_thread"] } },
    { id: "s1", space_id: null, thread_id: T2, detail: { kind: "secret", name: "FORGEJO_TOKEN", label: "Forgejo token", reason: "To publish", expires_at_ms: 1_800_000 } },
    { id: "z", space_id: SPACE, thread_id: T1, detail: { kind: "something-new" } },
  ] };
  test("the list is account-wide (no space), mapped to the three kinds the bar draws; unknown kinds are dropped", async () => {
    const d = double({ "requests.list": () => list });
    const r = await d.client.requests();
    expect(r.map((x) => [x.id, x.kind, x.threadId])).toEqual([["q1", "question", T1], ["a1", "approval", T1], ["a2", "approval", T1], ["s1", "secret", T2]]);
    expect(r[0]).toMatchObject({ title: "Before I open the pull request", form: "Form([],\"x\")" });
    expect(r[3]).toMatchObject({ name: "FORGEJO_TOKEN", expiresAt: 1_800_000, scope: "personal" });
    expect(d.ops("requests.list")[0]!.input).toEqual({});
  });
  test("the lifetimes come from the server's list, in its order; none listed means once only", async () => {
    const d = double({ "requests.list": () => list });
    const r = (await d.client.requests()).filter((x) => x.kind === "approval") as { lifetimes: string[] }[];
    expect(r.map((x) => x.lifetimes)).toEqual([["once"], ["once", "thread"]]);
    expect(lifetimesOf({ kind: "approval", app: "a", write: true, lifetimes: ["this_thread", "once"] })).toEqual(["thread", "once"]);
    expect(lifetimesOf({ kind: "approval", app: "a", write: true, lifetimes: [] })).toEqual(["once"]);
  });
  test("answers become the real operations: answer, approve once or this_thread, deny", async () => {
    const d = double({ "requests.list": () => ({ requests: [] }), "requests.answer": () => ({}), "requests.approve": () => ({}), "requests.deny": () => ({}) });
    d.client.watchRequests(() => {});
    await d.client.answer("q1", { kind: "question", values: { branch: "x" } });
    await d.client.answer("a1", { kind: "approval", decision: "once" });
    await d.client.answer("a2", { kind: "approval", decision: "thread" });
    await d.client.answer("a3", { kind: "approval", decision: "deny" });
    expect(d.calls.filter((c) => c.operation !== "requests.list").map((c) => [c.operation, c.input])).toEqual([
      ["requests.answer", { id: "q1", answer: { branch: "x" } }],
      ["requests.approve", { id: "a1", lifetime: "once" }],
      ["requests.approve", { id: "a2", lifetime: "this_thread" }],
      ["requests.deny", { id: "a3" }],
    ]);
  });
  test("a secret's value goes to the trusted endpoint, with its space; skipping hides the prompt and calls nothing", async () => {
    const posts: { url: string; body: any }[] = [];
    const fetch = (async (url: string, init?: RequestInit) => {
      if (url.includes("/auth/secrets/provide")) { posts.push({ url, body: JSON.parse(init!.body as string) }); return new Response(JSON.stringify({ secret: { name: "X", version: 1 } }), { status: 200 }); }
      const b = JSON.parse(init!.body as string);
      return ok(b.operation === "requests.list" ? { requests: [{ id: "s2", space_id: SPACE, thread_id: T1, detail: { kind: "secret", name: "N", label: "L", reason: "R", expires_at_ms: 1 } }, { id: "s3", space_id: null, thread_id: T1, detail: { kind: "secret", name: "M", label: "L", reason: "R", expires_at_ms: 1 } }] } : {});
    }) as never;
    const c = new ServerThreads(new ServerClient({ token: () => "T", fetch }), () => SPACE);
    await c.requests();
    await c.answer("s2", { kind: "secret", value: "hunter2" });
    await c.answer("s3", { kind: "secret-skip" });
    expect(posts).toEqual([{ url: "/api/v1/auth/secrets/provide", body: { request_id: "s2", space_id: SPACE, value: "hunter2" } }]);
    expect((await c.requests()).map((r) => r.id)).toEqual(["s2"]);
  });
  test("a server without request operations answers not_found once: an empty list, and no more asking", async () => {
    const d = double({});
    expect(await d.client.requests()).toEqual([]);
    expect(await d.client.requests()).toEqual([]);
    expect(d.calls.filter((c) => c.operation === "requests.list")).toHaveLength(1);
  });
  test("a refused approval (no fresh passkey proof) is thrown with the server's words", async () => {
    const d = double({ "requests.approve": () => new Response(JSON.stringify({ request_id: "r", ok: false, error: { code: "passkey_reverification_required", message: "verify with your passkey again first" } }), { status: 403 }) });
    await expect(d.client.answer("a1", { kind: "approval", decision: "once" })).rejects.toMatchObject({ message: "verify with your passkey again first" });
    expect(d.reverified).toHaveLength(1);
  });
  test("a stale passkey proof asks for the passkey once and tries the approval again", async () => {
    let n = 0;
    const d = double({ "requests.approve": () => (n++ === 0 ? new Response(JSON.stringify({ request_id: "r", ok: false, error: { code: "passkey_reverification_required", message: "sign in again with a passkey to approve access" } }), { status: 403 }) : { id: "a1", status: "approved" }), "requests.list": () => ({ requests: [] }) });
    await d.client.answer("a1", { kind: "approval", decision: "once" });
    expect(d.reverified).toHaveLength(1);
    expect(d.ops("requests.approve")).toHaveLength(2);
  });
});

describe("checkpoint additions", () => {
  const body = (s: string) => new TextEncoder().encode(s.split("\n---\n")[1]!).length;
  test("an addition inside a message and one that opens a new message are spliced by byte offset, multi-byte text included", () => {
    const open = `---\ntitle: "x"\n---\n\n<!-- message:${M1} role:person time:2026-11-17T12:01:00.000Z actor:x -->\nhéllo 🌏\n<!-- /message -->\n`;
    const b = bodyOf(open);
    const first = parseMessages(open);
    // a new agent message at the end of the body
    const start = `\n<!-- message:${M2} role:agent time:2026-11-17T12:04:00.000Z turn:${JOB} actor:y -->\nPart one`;
    const r1 = applyAppend(b, first, b.len, start, 0, 0)!;
    expect(r1.messages.map((m) => m.id)).toEqual([M1, M2]);
    expect((r1.messages[1]!.blocks[0] as { text: string }).text).toBe("Part one");
    // then text inside that message, before its (not yet written) end
    const r2 = applyAppend(r1.body, r1.messages, r1.body.len, " and two.\n<!-- /message -->\n", 0, 0)!;
    expect((r2.messages[1]!.blocks[0] as { text: string }).text).toBe("Part one and two.");
    expect((r2.messages[0]!.blocks[0] as { text: string }).text).toBe("héllo 🌏");
    // the same file read whole gives the same messages
    const whole = parseMessages(open + start + " and two.\n<!-- /message -->\n");
    expect(r2.messages.map((m) => m.blocks)).toEqual(whole.map((m) => m.blocks));
    expect(applyAppend(b, first, 9999, "x", 0, 0)).toBeNull();
    void body;
  });
  test("saved tool lines become one steps group with the tool and its target; words between steps split the group", () => {
    const m = blocksOf("m", "Hello\n\n> Tool Bash · Run ls (started)\n\n> Tool Bash · Run ls (22 bytes of tool output)\n\n> Tool Bash · Run ls (finished)\n\n> Tool Read · Read ./a (started)\n\n> Tool Read · Read ./a (failed)\n\nThen this.\n\n> Tool Grep · Search x (started)");
    expect(m.map((b) => b.type)).toEqual(["text", "steps", "text", "steps"]);
    const g1 = (m[1] as { steps: { title: string; detail?: string; status: string; kind: string; output?: string }[] }).steps;
    expect(g1.map((s) => [s.detail, s.title, s.status, s.kind])).toEqual([["Bash", "Run ls", "ok", "run"], ["Read", "Read ./a", "failed", "read"]]);
    expect(g1[0]!.output).toBe("22 bytes of output");
    expect((m[3] as { steps: { status: string }[] }).steps[0]!.status).toBe("running");
  });
});

describe("models, phases and reasoning (docs/threads.md, \"Models, phases and turn history\")", () => {
  test("the picker's providers come from threads.models: models, the levels each accepts, the default; a server without it says so", async () => {
    const d = double({ "threads.models": () => ({ providers: [{ id: "calternal-claude-code", name: "Claude Code", default_model: "claude-sonnet-5-5", models: [{ id: "claude-opus-5-5", name: "Opus 5.5", reasoning_levels: ["low", "high"] }, { id: "claude-sonnet-5-5", name: "Sonnet 5.5", reasoning_levels: ["low", "medium", "high"] }, { id: "claude-haiku-5-5", name: "Haiku 5.5" }] }, { id: "some-other-plugin", name: "?", default_model: "x", models: [] }] }) });
    const p = await d.client.providers();
    expect(d.ops("threads.models")[0]!.input).toEqual({ space_id: SPACE });
    expect(p).toHaveLength(1);
    expect(p[0]).toMatchObject({ id: "claude", name: "Claude Code", listed: true });
    expect(p[0]!.models.map((m) => [m.id, m.levels ?? null, !!m.default])).toEqual([["claude-opus-5-5", ["low", "high"], false], ["claude-sonnet-5-5", ["low", "medium", "high"], true], ["claude-haiku-5-5", null, false]]);
    const old = await double({}).client.providers();
    expect(old[0]).toMatchObject({ listed: false });
  });
  test("a send carries the chosen model and reasoning level", async () => {
    const d = double({ "threads.get": () => thread(md(), "r1"), "threads.send": () => ({ ...thread(md(), "r2"), job: { id: JOB, state: "queued" } }) });
    d.setEvents(sse([], 20));
    await d.client.setProvider(T1, "claude", "claude-sonnet-5-5"); await d.client.setReasoning(T1, "high");
    await d.client.send(T1, "hi", "req");
    expect(d.ops("threads.send")[0]!.input).toMatchObject({ model: "claude-sonnet-5-5", reasoning: "high" });
  });
  test("the job stream's turn phases and reasoning events reach the working row", async () => {
    const d = double({ "threads.get": () => thread(md(), "r1"), "threads.send": () => ({ ...thread(md(), "r2"), job: { id: JOB, state: "queued" } }) });
    d.setEvents(sse([
      ["chunk", { seq: 0, data: { type: "turn_phase", phase: "queued", at_ms: 1 } }],
      ["chunk", { seq: 1, data: { type: "turn_phase", phase: "starting_sandbox", at_ms: 2 } }],
      ["chunk", { seq: 2, data: { type: "turn_phase", phase: "thinking", at_ms: 3 } }],
      ["chunk", { seq: 3, data: { version: 1, seq: 1, event: { type: "reasoning_started" } } }],
      ["chunk", { seq: 4, data: { version: 1, seq: 2, event: { type: "reasoning_delta", text: "plan " } } }],
      ["chunk", { seq: 5, data: { version: 1, seq: 3, event: { type: "reasoning_finished", duration_ms: 4200 } } }],
    ], 100));
    const got: string[] = [];
    const ev: ThreadEvents = { onMessage() {}, onStreamStart() {}, onDelta() {}, onStep() {}, onSummary() {}, onPhase: (p) => got.push(`phase ${p}`), onThought: (t) => got.push(`thought ${t.text}${t.done ? ` done ${t.ms}` : ""}`) };
    await d.client.getThread(T1);
    d.client.watch(T1, ev);
    await until(() => d.hasFeed(), "the feed");
    await d.client.send(T1, "hi", "req");
    await until(() => got.some((x) => x.includes("done")), "the reasoning to finish");
    expect(got.filter((x) => x.startsWith("phase")).slice(0, 4)).toEqual(["phase Waiting to start", "phase Waiting to start", "phase Starting the machine", "phase Thinking"]);
    expect(got.filter((x) => x.startsWith("thought"))).toEqual(["thought ", "thought plan ", "thought  done 4200"]);
  });
});
