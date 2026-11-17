// The server adapter against a recorded-response double: the response shapes are the ones `crates/server/src/threads` returns (docs/threads.md), the job events are
// the SSE frames `job_api.rs` writes (provider events nested as chunk.data.event, docs/dev-e2e.md), the journal entries are the kinds docs/threads.md lists. No network.
import { describe, expect, test } from "vitest";
import { ServerClient } from "./client";
import { ServerThreads } from "./threads-client";
import { parseMessages, stateOfTurn, summaryOf } from "./thread-file";
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
  const client = new ServerThreads(new ServerClient({ token: () => "T", fetch }), () => SPACE);
  const push = (kind: string, item: string, data?: object) => live!.enqueue(frame("message", { type: "entry", space_id: SPACE, entry: { kind, item, data } }));
  return { client, calls, eventUrls, push, setEvents: (r: Response) => (events = r), ops: (name: string) => calls.filter((c) => c.operation === name) };
}

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
    await wait(60);
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
    await wait(40);
    const lists = d.ops("threads.list").length;
    d.push("threads.turn_state_changed", T1, { job_id: JOB, state: "running", job_state: "running" });
    await wait(60);
    off();
    expect(seen).toContain("running");
    expect(d.ops("threads.list")).toHaveLength(lists);
  });
  test("a created, renamed or archived thread re-reads the list once, however many entries arrive", async () => {
    const d = double({ "threads.list": () => ({ threads: [row(T1)] }) });
    const off = d.client.watchThreads(() => {});
    await wait(40);
    const lists = d.ops("threads.list").length;
    d.push("threads.rename", T1); d.push("threads.archive", T2); d.push("threads.create", T2);
    await wait(300);
    off();
    expect(d.ops("threads.list")).toHaveLength(lists + 1);
  });
  test("a checkpoint of the open thread re-reads that thread and tells the view the changed message", async () => {
    let rev = 1;
    const d = double({ "threads.get": () => (rev === 1 ? thread(md(), "r1") : thread(md(agent), "r2")), "threads.list": () => ({ threads: [row(T1)] }) });
    const got: string[] = [];
    const off = d.client.watch(T1, { onMessage: (m, i) => got.push(`${i} ${m.role}`), onStreamStart() {}, onDelta() {}, onStep() {}, onSummary() {} });
    await wait(80);
    rev = 2;
    d.push("threads.checkpoint", T1);
    await wait(400);
    off();
    expect(got).toContain("1 agent");
  });
  test("a reloaded page follows the running turn the thread names, and resumes from the last event it saw", async () => {
    const running = { ...thread(md(), "r1", {}, "running"), job: { id: JOB, state: "running" } };
    const d = double({ "threads.get": () => running, "threads.list": () => ({ threads: [row(T1, "running")] }) });
    d.setEvents(sse([["chunk", { seq: 1, data: { version: 1, seq: 1, event: { type: "tool_started", summary: "Read a" } } }]], 5000));
    const started: string[] = [];
    const off = d.client.watch(T1, { onMessage() {}, onStreamStart: (id) => started.push(id), onDelta() {}, onStep() {}, onSummary() {} });
    await wait(200);
    off();
    expect(started).toEqual([JOB]);
    expect(d.eventUrls[0]).toContain(JOB);
  });
  test("send carries the composer's mode, provider and model, with the caller's request id, and follows the job: steps, then the saved answer", async () => {
    let revision = 0;
    const d = double({
      "threads.get": () => (revision ? thread(md(agent), "r2") : thread(md(), "r1")),
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
    await wait(30);
    await d.client.setPermission(T1, "full");
    await d.client.setProvider(T1, "claude", "sonnet-5-5");
    setTimeout(() => (revision = 1), 20); // the server's file has the answer once the turn ends
    await d.client.send(T1, "Inspect this workspace.", "req-1");
    await wait(350);
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

describe("requests (issue #32's operations; the wire shape is the lane's stated goal)", () => {
  const list = { requests: [
    { id: "q1", kind: "question", thread_id: T1, title: "Before I open the pull request", form: "Form([],\"x\")" },
    { id: "a1", kind: "approval", thread_id: T1, title: "Run a command", action: "Push the branch", command: "git push origin lane/x" },
    { id: "s1", kind: "secret", thread_id: T2, name: "FORGEJO_TOKEN", label: "Forgejo token", why: "To publish", scope: "personal", required: true, expires_ms: 1_800_000 },
    { id: "z", kind: "something-new", thread_id: T1 },
  ] };
  test("a list maps to the three kinds the bar draws and drops what it cannot", async () => {
    const d = double({ "requests.list": () => list });
    const r = await d.client.requests();
    expect(r.map((x) => [x.id, x.kind, x.threadId])).toEqual([["q1", "question", T1], ["a1", "approval", T1], ["s1", "secret", T2]]);
    expect(r[2]).toMatchObject({ name: "FORGEJO_TOKEN", expiresAt: 1_800_000, scope: "personal" });
    expect(d.calls[0]).toMatchObject({ operation: "requests.list", input: { space_id: SPACE } });
  });
  test("answers become the operations: answer, approve once or for the thread, deny; a secret's value goes in the answer", async () => {
    const d = double({ "requests.list": () => ({ requests: [] }), "requests.answer": () => ({}), "requests.approve": () => ({}), "requests.deny": () => ({}) });
    await d.client.answer("q1", { kind: "question", values: { branch: "x" } });
    await d.client.answer("a1", { kind: "approval", decision: "once" });
    await d.client.answer("a2", { kind: "approval", decision: "thread" });
    await d.client.answer("a3", { kind: "approval", decision: "deny" });
    await d.client.answer("s1", { kind: "secret", value: "hunter2" });
    await d.client.answer("s2", { kind: "secret-skip" });
    expect(d.calls.filter((c) => c.operation !== "requests.list").map((c) => [c.operation, c.input])).toEqual([
      ["requests.answer", { id: "q1", values: { branch: "x" } }],
      ["requests.approve", { id: "a1", lifetime: "once" }],
      ["requests.approve", { id: "a2", lifetime: "thread" }],
      ["requests.deny", { id: "a3" }],
      ["requests.answer", { id: "s1", secret: "hunter2" }],
      ["requests.deny", { id: "s2" }],
    ]);
  });
  test("after an answer the watchers hear the new list", async () => {
    let open = true;
    const d = double({ "requests.list": () => ({ requests: open ? [list.requests[0]] : [] }), "requests.approve": () => { open = false; return {}; } });
    const seen: number[] = [];
    const off = d.client.watchRequests((r) => seen.push(r.length));
    await d.client.answer("a1", { kind: "approval", decision: "once" });
    off();
    expect(seen).toEqual([0]);
  });
  test("a server without request operations answers not_found once: an empty list, and no more asking", async () => {
    const d = double({});
    expect(await d.client.requests()).toEqual([]);
    expect(await d.client.requests()).toEqual([]);
    expect(d.calls.filter((c) => c.operation === "requests.list")).toHaveLength(1);
  });
  test("a refused answer is thrown as the server said it", async () => {
    const d = double({ "requests.approve": () => fail("forbidden", "an agent cannot approve") });
    await expect(d.client.answer("a1", { kind: "approval", decision: "once" })).rejects.toBeDefined();
  });
});
