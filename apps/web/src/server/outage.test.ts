import { beforeEach, describe, expect, test } from "vitest";
import { ServerClient, ServerError } from "./client";
import { link } from "./link.svelte";

// The server is away for a while (an update: the proxy answers 502, 503 or 504; or the connection drops): the shell shows the reconnecting state, the call waits and tries again with its ORIGINAL request id until the
// server answers, and the answer is the first one the server gave (its receipts make a repeat safe). Only the server being away is retried; its own refusal is not.
const ok = (data: unknown = {}) => new Response(JSON.stringify({ ok: true, data }), { status: 200 });
const html = (status: number) => new Response("<html><body>Bad gateway</body></html>", { status, headers: { "content-type": "text/html" } });

function away(first: () => Response | Promise<Response>, times: number) {
  const ids: string[] = [], states: string[] = [];
  let calls = 0;
  const c = new ServerClient({
    token: () => null,
    retry: { first: 1, max: 2, total: 2000 },
    fetch: (async (_u: string, init?: RequestInit) => {
      calls++;
      if (init?.body) ids.push((JSON.parse(String(init.body)) as { request_id: string }).request_id);
      states.push(link.state);
      if (calls <= times) return first();
      return ok({ note: "saved" });
    }) as typeof fetch,
  });
  return { c, ids, states, calls: () => calls };
}

beforeEach(() => { link.state = "ok"; });

describe("the server away: a proxy's 502, 503 and 504, and a dropped connection", () => {
  test.each([502, 503, 504])("%i: the call waits, is repeated with the same request id, and ends with the server's answer", async (status) => {
    const { c, ids, states } = away(() => html(status), 3);
    const out = await c.operate("notes.update", { id: "n" });
    expect(out).toEqual({ note: "saved" });
    expect(ids).toHaveLength(4);
    expect(new Set(ids).size).toBe(1);
    expect(states[1]).toBe("reconnecting"); // the shell shows it while the server is away
    expect(link.state).toBe("ok"); // and settles once it answers
  });
  test("the server's own JSON answer with a 503 is an answer, not an outage (no retry, no reconnecting)", async () => {
    const { c, ids, states } = away(() => new Response(JSON.stringify({ ok: false, error: { code: "conflict", message: "no machine to run on" } }), { status: 503, headers: { "content-type": "application/json" } }), 5);
    await expect(c.operate("automations.run_now", { id: "a" })).rejects.toMatchObject({ code: "conflict", status: 503 });
    expect(ids).toHaveLength(1);
    expect(states).not.toContain("reconnecting");
  });
  test("a dropped connection is the same", async () => {
    const { c, ids } = away(() => { throw new TypeError("Failed to fetch"); }, 2);
    expect(await c.operate("notes.update", { id: "n" })).toEqual({ note: "saved" });
    expect(new Set(ids).size).toBe(1);
    expect(ids).toHaveLength(3);
  });
  test("a caller's own request id is the one that is repeated", async () => {
    const { c, ids } = away(() => html(502), 2);
    await c.operate("notes.update", { id: "n" }, "req-fixed-1");
    expect(ids).toEqual(["req-fixed-1", "req-fixed-1", "req-fixed-1"]);
  });
  test("the same for a read", async () => {
    const { c, ids } = away(() => html(503), 1);
    await c.operate("notes.list", { space_id: "s" });
    expect(ids).toHaveLength(2);
  });
  test("a route that is not an operation sees the server away as unavailable (not an unreadable answer)", async () => {
    const { c } = away(() => html(502), 5);
    await expect(c.json("/api/v1/machines")).rejects.toMatchObject({ code: "unavailable", status: 502 });
  });
  test("the patience runs out: the caller gets unavailable (the app then keeps the change queued)", async () => {
    const c = new ServerClient({ token: () => null, retry: { first: 1, max: 2, total: 40 }, fetch: (async () => html(502)) as typeof fetch });
    await expect(c.operate("notes.update", { id: "n" })).rejects.toMatchObject({ code: "unavailable" });
  });
});

describe("the server's own answers are never retried", () => {
  test.each([
    ["a refusal in its envelope", () => new Response(JSON.stringify({ ok: false, error: { code: "conflict", message: "stale" } }), { status: 409 }), "conflict"],
    ["an error of its own", () => new Response(JSON.stringify({ ok: false, error: { code: "internal", message: "oops" } }), { status: 500 }), "internal"],
    ["a 500 that is not an envelope", () => new Response("boom", { status: 500 }), "internal"],
    ["a 404", () => new Response(JSON.stringify({ ok: false, error: { code: "not_found", message: "" } }), { status: 404 }), "not_found"],
  ] as const)("%s", async (_n, make, code) => {
    let calls = 0;
    const c = new ServerClient({ token: () => null, retry: { first: 1, max: 2, total: 2000 }, fetch: (async () => { calls++; return make(); }) as typeof fetch });
    await expect(c.operate("notes.update", { id: "n" })).rejects.toMatchObject({ code });
    expect(calls).toBe(1);
    expect(ServerError).toBeDefined();
  });
});
