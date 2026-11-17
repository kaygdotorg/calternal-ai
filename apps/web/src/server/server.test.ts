import { describe, expect, test } from "vitest";
import { ServerClient, ServerError } from "./client";
import { events, parseBlock } from "./sse";

const enc = new TextEncoder();
const body = (...chunks: string[]) => new ReadableStream<Uint8Array>({ start(c) { for (const x of chunks) c.enqueue(enc.encode(x)); c.close(); } });
const json = (v: unknown, status = 200) => new Response(JSON.stringify(v), { status, headers: { "content-type": "application/json" } });

describe("operations", () => {
  test("posts the envelope with the bearer and returns the data", async () => {
    let seen: { url: string; init: RequestInit } | null = null;
    const c = new ServerClient({ token: () => "T", fetch: (async (url: string, init: RequestInit) => { seen = { url, init }; return json({ request_id: "r", ok: true, data: { spaces: [{ id: "s1", name: "Home" }] } }); }) as never });
    const r = await c.spaces.list();
    expect(r.spaces[0]?.name).toBe("Home");
    expect(seen!.url).toBe("/api/v1/operations");
    expect((seen!.init.headers as Record<string, string>).authorization).toBe("Bearer T");
    const sent = JSON.parse(seen!.init.body as string);
    expect(sent.operation).toBe("spaces.list");
    expect(sent.request_id).toMatch(/[0-9a-f-]{36}/);
  });
  test("a failure envelope becomes a ServerError with the server's code", async () => {
    const c = new ServerClient({ token: () => null, fetch: (async () => json({ request_id: "r", ok: false, error: { code: "conflict", message: "stale" } }, 409)) as never });
    await expect(c.notes.update({ id: "n", base_revision: "1", markdown: "x" } as never)).rejects.toMatchObject({ code: "conflict", message: "stale", status: 409 });
  });
  test("describe answers without an envelope", async () => {
    const c = new ServerClient({ token: () => "T", fetch: (async () => json({ protocol_version: 1, genui_version: 1, principal: { id: "p", spaces: [] }, operations: [{ id: "spaces.list" }] })) as never });
    expect((await c.describe()).operations).toHaveLength(1);
  });
  test("a network failure is unavailable", async () => {
    const c = new ServerClient({ token: () => null, fetch: (async () => { throw new Error("offline"); }) as never });
    await expect(c.describe()).rejects.toBeInstanceOf(ServerError);
  });
  test("a retry keeps its request id", async () => {
    const ids: string[] = [];
    const c = new ServerClient({ token: () => null, fetch: (async (_u: string, init: RequestInit) => { ids.push(JSON.parse(init.body as string).request_id); return json({ request_id: ids[0], ok: true, data: {} }); }) as never });
    await c.notes.create({ space_id: "s", title: "t", markdown: "m" }, "same");
    await c.notes.create({ space_id: "s", title: "t", markdown: "m" }, "same");
    expect(ids).toEqual(["same", "same"]);
  });
});

describe("server-sent events", () => {
  test("parses event, id and data across chunk boundaries", async () => {
    const out = [];
    for await (const e of events(body("event: snapshot\nid: 1:0\nda", 'ta: {"a":1}\n\nevent: chunk\ndata: x\ndata: y\n\n:keepalive\n\n'))) out.push(e);
    expect(out).toEqual([{ event: "snapshot", id: "1:0", data: '{"a":1}' }, { event: "chunk", id: undefined, data: "x\ny" }]);
  });
  test("parseBlock ignores comments", () => expect(parseBlock(": hi")).toBeNull());
});

describe("the live stream", () => {
  test("delivers entries, ready and reset frames in the generated shape", async () => {
    const frames: unknown[] = [];
    const sse = 'data: {"type":"entry","space_id":"s","entry":{"seq":1}}\n\ndata: {"type":"ready","space_id":"s","bookmark":"b"}\n\ndata: {"type":"reset","space_id":"s"}\n\n';
    let url = "";
    const c = new ServerClient({ token: () => "T", fetch: (async (u: string) => { url = u; return new Response(body(sse), { headers: { "content-type": "text/event-stream" } }); }) as never });
    await c.live([{ space_id: "s", bookmark: null }], (f) => frames.push(f));
    expect(url).toContain("/api/v1/live?subscriptions=");
    expect(JSON.parse(decodeURIComponent(url.split("subscriptions=")[1]!))).toEqual([{ space_id: "s", bookmark: null }]);
    expect(frames).toEqual([{ Entry: { type: "entry", space_id: "s", entry: { seq: 1 } } }, { Ready: { type: "ready", space_id: "s", bookmark: "b" } }, { Reset: { type: "reset", space_id: "s" } }]);
  });
});

describe("the jobs stream", () => {
  test("snapshot, chunks and end; a missing job is not_found", async () => {
    const got: string[] = [];
    const c = new ServerClient({ token: () => null, fetch: (async () => new Response(body('event: snapshot\ndata: {"state":"running"}\n\nevent: chunk\nid: 1:1\ndata: {"seq":1,"data":"a"}\n\nevent: end\ndata: {"state":"done"}\n\n')) ) as never });
    await c.jobEvents("j", (n) => got.push(n));
    expect(got).toEqual(["snapshot", "chunk", "end"]);
    const gone = new ServerClient({ token: () => null, fetch: (async () => new Response("", { status: 404 })) as never });
    await expect(gone.jobEvents("x", () => {})).rejects.toMatchObject({ code: "not_found" });
  });
});

describe("drafts, pending submissions and the account feed", () => {
  test("drafts and pending operations use the documented inputs", async () => {
    const seen: { operation: string; input: unknown }[] = [];
    const c = new ServerClient({ token: () => "T", fetch: (async (_u: string, init: RequestInit) => { seen.push(JSON.parse(init.body as string)); return json({ request_id: "r", ok: true, data: { record: null, result: null } }); }) as never });
    await c.drafts.save("s", "i", "text");
    await c.pending.queue("s", "i", 5, { request_id: "q", operation: "notes.update", input: { id: "i" } });
    await c.pending.submit("q");
    expect(seen.map((x) => x.operation)).toEqual(["drafts.save", "pending.queue", "pending.submit"]);
    expect(seen[0]!.input).toEqual({ space_id: "s", item_id: "i", text: "text" });
    expect(seen[1]!.input).toMatchObject({ space_id: "s", item_id: "i", created_at_ms: 5, request: { request_id: "q" } });
    expect(seen[2]!.input).toEqual({ id: "q" });
  });
  test("the account feed delivers changed, ready and reset", async () => {
    const frames: unknown[] = [];
    const sse = 'data: {"type":"changed","bookmark":"b1"}\n\ndata: {"type":"ready","bookmark":"b2"}\n\ndata: {"type":"reset"}\n\n';
    const c = new ServerClient({ token: () => "T", fetch: (async () => new Response(body(sse))) as never });
    await c.accountLive((f) => frames.push(f));
    expect(frames).toEqual([{ Changed: { bookmark: "b1" } }, { Ready: { bookmark: "b2" } }, "Reset"]);
  });
});
