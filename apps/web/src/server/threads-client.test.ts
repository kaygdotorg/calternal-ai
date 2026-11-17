// The server adapter against a recorded-response double: the response shapes are the ones `crates/server/src/threads` returns (docs/threads.md), the job events are
// the SSE frames `job_api.rs` writes. No network.
import { describe, expect, test } from "vitest";
import { ServerClient } from "./client";
import { ServerThreads, Unsupported } from "./threads-client";
import { parseMessages, stateOfJob, summaryOf } from "./thread-file";
import type { ThreadEvents } from "../apps/threads/client";

const SPACE = "0197f3a2-5c1e-7b4d-9a60-3d2e8f1c4b11";
const T1 = "019a8c00-0000-7000-8000-000000000001";
const T2 = "019a8c00-0000-7000-8000-000000000002";
const JOB = "job-0000000000000001";
const M1 = "019a8c00-0000-7000-8000-0000000000a1";
const M2 = "019a8c00-0000-7000-8000-0000000000a2";
const md = (extra = "") => `---\ntitle: "Plan"\nprovider: "calternal-claude-code"\nmode: "approve_for_me"\ncreated-at: "2026-11-17T12:00:00.000Z"\nupdated-at: 2026-11-17T12:05:00.000Z\n---\n\n<!-- message:${M1} role:person actor:x -->\nInspect this workspace.\n<!-- /message -->\n${extra}`;
const agent = `\n<!-- message:${M2} role:agent turn:${JOB} actor:y -->\nI looked. Nothing wrong.\n<!-- /message -->\n`;
const meta = (o: object = {}) => ({ title: "Plan", provider: "calternal-claude-code", mode: "approve_for_me", mode_for: "acc", created_at: "2026-11-17T12:00:00.000Z", archived: false, deleted: false, ...o });
const thread = (markdown: string, revision = "r1", o: object = {}) => ({ thread: { id: T1, space_id: SPACE, address: `/threads/${T1}`, revision, metadata: meta(o), markdown } });
const enc = new TextEncoder();
const sse = (...frames: [string, unknown][]) => new Response(new ReadableStream<Uint8Array>({ start(c) { for (const [n, d] of frames) c.enqueue(enc.encode(`event: ${n}\ndata: ${JSON.stringify(d)}\n\n`)); setTimeout(() => c.close(), 40); } }), { headers: { "content-type": "text/event-stream" } });
const ok = (data: unknown) => new Response(JSON.stringify({ request_id: "r", ok: true, data }), { headers: { "content-type": "application/json" } });
const fail = (code: string, message: string) => new Response(JSON.stringify({ request_id: "r", ok: false, error: { code, message } }), { status: 404 });

/** A double of the server: records every operation, answers from `handlers`. */
function double(handlers: Record<string, (input: any, requestId: string) => unknown>) {
  const calls: { operation: string; input: any; requestId: string }[] = [];
  let events: Response | null = null;
  const fetch = (async (url: string, init?: RequestInit) => {
    if (url.includes("/jobs/")) return events ?? fail("not_found", "no job");
    const b = JSON.parse(init!.body as string);
    calls.push({ operation: b.operation, input: b.input, requestId: b.request_id });
    const h = handlers[b.operation];
    return h ? (() => { const r = h(b.input, b.request_id); return r instanceof Response ? r : ok(r); })() : fail("not_found", "unknown operation");
  }) as never;
  const client = new ServerThreads(new ServerClient({ token: () => "T", fetch }), () => SPACE);
  return { client, calls, setEvents: (r: Response) => (events = r) };
}

describe("the saved thread's Markdown", () => {
  test("messages are read in order, with roles and ids; unclosed ones run to the next marker", () => {
    const ms = parseMessages(md(agent), 1000, 2000);
    expect(ms.map((m) => [m.id, m.role])).toEqual([[M1, "user"], [M2, "agent"]]);
    expect(ms[0]!.blocks).toEqual([{ type: "text", text: "Inspect this workspace." }]);
    expect(ms[1]!.at).toBe(2000);
    const open = parseMessages(`---\ntitle: "x"\n---\n<!-- message:${M1} role:agent -->\nhalf wri`);
    expect(open).toHaveLength(1);
    expect((open[0]!.blocks[0] as { text: string }).text).toBe("half wri");
  });
  test("message text that looks like a marker inside a body cannot add a role: only real markers split", () => {
    const ms = parseMessages(`---\ntitle: "x"\n---\n<!-- message:${M1} role:person -->\nsay <!-- hello --> ok\n<!-- /message -->\n`);
    expect(ms).toHaveLength(1);
  });
  test("job states map to the three that mean something", () => {
    expect(["queued", "running", "retry_waiting"].map(stateOfJob)).toEqual(["running", "running", "running"]);
    expect(stateOfJob("interrupted")).toBe("waiting");
    expect(stateOfJob("failed")).toBe("failed");
    expect(stateOfJob("succeeded")).toBe("idle");
  });
  test("a summary maps the provider plugin and the mode", () => {
    const s = summaryOf(T1, SPACE, meta({ mode: "full_access", provider: "calternal-codex", parent_thread: T2 }));
    expect([s.provider, s.permission, s.parentId]).toEqual(["codex", "full", T2]);
  });
});

describe("ServerThreads", () => {
  test("lists a space's threads, children included", async () => {
    const d = double({ "threads.list": () => ({ threads: [{ id: T1, address: "", revision: "r", metadata: meta() }, { id: T2, address: "", revision: "r", metadata: meta({ title: "Child", parent_thread: T1 }) }] }) });
    const l = await d.client.listThreads();
    expect(l.map((t) => [t.id, t.parentId])).toEqual([[T1, null], [T2, T1]]);
    expect(d.calls.find((c) => c.operation === "threads.list")).toMatchObject({ input: { space_id: SPACE } });
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
  test("send carries the composer's mode, provider and model, with the caller's request id, and follows the job: steps, then the saved answer", async () => {
    let revision = 0;
    const d = double({
      "threads.get": () => (revision ? thread(md(agent), "r2") : thread(md(), "r1")),
      "threads.send": () => ({ ...thread(md(), "r1b"), job: { id: JOB, state: "queued" } }),
    });
    d.setEvents(sse(
      ["snapshot", { job: { id: JOB, state: "running" }, epoch: "e" }],
      ["chunk", { seq: 0, data: { type: "placement", note: "not a provider event" } }],
      ["chunk", { seq: 1, data: { version: 1, seq: 1, event: { type: "tool_started", summary: "Read src/lib.rs" } } }],
      ["chunk", { seq: 2, data: { version: 1, seq: 2, event: { type: "tool_finished", failed: false } } }],
      ["chunk", { seq: 3, data: { version: 1, seq: 3, event: { type: "text_delta", text: "I looked." } } }],
    ));
    const got: string[] = [];
    const ev: ThreadEvents = { onMessage: (m, i) => got.push(`message ${i} ${m.role}`), onStreamStart: () => got.push("start"), onDelta: (t) => got.push(`delta ${t}`), onStep: (s) => got.push(`step ${s.title} ${s.status}`), onSummary: () => {}, onStreamEnd: () => got.push("end") };
    await d.client.getThread(T1);
    const off = d.client.watch(T1, ev);
    await d.client.setPermission(T1, "full");
    await d.client.setProvider(T1, "claude", "sonnet-5-5");
    setTimeout(() => (revision = 1), 20); // the server's file has the answer once the turn ends
    await d.client.send(T1, "Inspect this workspace.", "req-1");
    await new Promise((r) => setTimeout(r, 120));
    off();
    const send = d.calls.find((c) => c.operation === "threads.send")!;
    expect(send.requestId).toBe("req-1");
    expect(send.input).toEqual({ id: T1, space_id: SPACE, message: "Inspect this workspace.", mode: "full_access", provider: "calternal-claude-code", model: "sonnet-5-5" });
    expect(got).toEqual(["start", "step Read src/lib.rs ok", "delta I looked.", "end", "message 1 agent"]);
  });
  test("stop, archive and delete are one operation each, on the thread's own id", async () => {
    const d = double({ "threads.stop": () => thread(md()), "threads.archive": () => thread(md()), "threads.delete": () => thread(md()), "threads.list": () => ({ threads: [] }) });
    await d.client.stop(T1); await d.client.archive(T1); await d.client.deleteThread(T1);
    expect(d.calls.filter((c) => /^threads\.(stop|archive|delete)$/.test(c.operation)).map((c) => [c.operation, c.input.id])).toEqual([["threads.stop", T1], ["threads.archive", T1], ["threads.delete", T1]]);
  });
  test("create sends the title, provider and parent", async () => {
    const d = double({ "threads.create": () => thread(""), "threads.list": () => ({ threads: [] }) });
    const t = await d.client.createThread("Plan", { provider: "claude", parentId: T2 });
    expect(d.calls[0]!.input).toEqual({ space_id: SPACE, title: "Plan", provider: "calternal-claude-code", parent_thread: T2 });
    expect(t.id).toBe(T1);
  });
  test("what the server cannot do says so", async () => {
    const d = double({});
    await expect(d.client.rename()).rejects.toBeInstanceOf(Unsupported);
    expect(await d.client.unarchive()).toBe(false);
    expect(await d.client.machines()).toEqual([]);
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
