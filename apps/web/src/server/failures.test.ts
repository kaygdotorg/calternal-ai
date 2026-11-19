import { beforeEach, describe, expect, it, vi } from "vitest";
import { ServerClient, ServerError } from "./client";
import { clearCache, readCache, writeCache } from "./cache";
import { link } from "./link.svelte";
import * as people from "./people";

// The adapters' failure paths: the server unreachable, a session that ended in the middle of use, a feed that resets, a page cursor the server refuses, an answer that is not what the contract says.
const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
const client = (f: (url: string, init?: RequestInit) => Promise<Response>, token: string | null = null) => new ServerClient({ token: () => token, fetch: f as typeof fetch, retry: { first: 1, max: 1, total: 0 } }); // no patience: these tests are about the first answer (outage.test.ts is about waiting)
const stream = (frames: string[]) => new Response(new ReadableStream({ start(c) { for (const f of frames) c.enqueue(new TextEncoder().encode(`data: ${f}\n\n`)); c.close(); } }), { status: 200, headers: { "content-type": "text/event-stream" } });

beforeEach(() => { link.state = "ok"; link.quiet = false; localStorage.clear(); });

describe("operations", () => {
  it("a 429 refusal with its envelope (a cooldown) is the server's answer: not retried, code rate_limited, in words", async () => {
    const calls = vi.fn(async () => new Response(JSON.stringify({ request_id: "r", ok: false, error: { code: "rate_limited", message: "Register this browser or wait a minute before testing again.", details: { retry_after_seconds: 40 } } }), { status: 429, headers: { "content-type": "application/json", "retry-after": "40" } }));
    const c = client(calls);
    await expect(c.operate("push.test", {})).rejects.toMatchObject({ code: "rate_limited" });
    expect(calls).toHaveBeenCalledTimes(1);
  });
  it("an unreachable server is `unavailable`, in words that name no detail", async () => {
    const c = client(async () => { throw new TypeError("Failed to fetch"); });
    await expect(c.operate("notes.list", {})).rejects.toMatchObject({ code: "unavailable" });
    expect(link.state).toBe("reconnecting"); // the shell says so (and probes until it answers)
    link.ok();
  });
  it("a 401 in the middle of use ends the session for a cookie session, not for a development token", async () => {
    const expired = vi.spyOn(link, "expired").mockImplementation(() => {});
    await expect(client(async () => json(401, { ok: false, error: { code: "unauthorized", message: "x" } })).operate("notes.list", {})).rejects.toMatchObject({ code: "unauthorized" });
    expect(expired).toHaveBeenLastCalledWith(false);
    await expect(client(async () => json(401, { ok: false, error: { code: "unauthorized", message: "x" } }), "dev").operate("notes.list", {})).rejects.toBeInstanceOf(ServerError);
    expect(expired).toHaveBeenLastCalledWith(true); // told it is a token, which signing in again cannot fix
    expired.mockRestore();
  });
  it("an answer that is not JSON is `internal`, with the status, never a crash", async () => {
    const c = client(async () => new Response("<html>oops</html>", { status: 500 }));
    await expect(c.operate("notes.list", {})).rejects.toMatchObject({ code: "internal", status: 500 });
  });
  it("an operation the server refuses carries its own stable code", async () => {
    const c = client(async () => json(409, { ok: false, error: { code: "conflict", message: "stale" } }));
    await expect(c.operate("notes.update", {})).rejects.toMatchObject({ code: "conflict", status: 409 });
  });
});

describe("other routes", () => {
  it("a refusal without a body is named by its status, and 204 is an empty answer", async () => {
    await expect(client(async () => new Response("", { status: 403 })).json("/api/v1/x")).rejects.toMatchObject({ code: "forbidden" });
    await expect(client(async () => new Response("", { status: 404 })).json("/api/v1/x")).rejects.toMatchObject({ code: "not_found" });
    await expect(client(async () => new Response("", { status: 429 })).json("/api/v1/x")).rejects.toMatchObject({ code: "too_many_requests" });
    await expect(client(async () => new Response(null, { status: 204 })).json("/api/v1/x", { method: "POST", body: {} })).resolves.toEqual({});
  });
  it("a passkey the server refuses (401) is not a session that ended", async () => {
    const expired = vi.spyOn(link, "expired").mockImplementation(() => {});
    await expect(client(async () => json(401, { error: { code: "passkey_failed", message: "x" } })).json("/api/v1/auth/login/finish", { body: {}, link: false })).rejects.toMatchObject({ code: "passkey_failed" });
    expect(expired).not.toHaveBeenCalled();
    expired.mockRestore();
  });
});

describe("feeds", () => {
  it("a reset frame and a readiness frame both reach the reader in the generated form, whatever the wire spelling", async () => {
    const c = client(async () => stream(['{"type":"reset"}', '{"type":"readiness_changed"}', '"Reset"', '{"type":"ready","bookmark":"b1"}', '{"Changed":{"bookmark":"b2"}}']));
    const got: unknown[] = [];
    await c.accountLive((f) => got.push(f));
    expect(got).toEqual(["Reset", "ReadinessChanged", "Reset", { Ready: { bookmark: "b1" } }, { Changed: { bookmark: "b2" } }]);
  });
  it("a stream the server refuses is a failure the caller can tell apart: gone, or away", async () => {
    await expect(client(async () => new Response("", { status: 404 })).jobEvents("j", () => {})).rejects.toMatchObject({ code: "not_found" });
    await expect(client(async () => new Response("", { status: 500 })).accountLive(() => {})).rejects.toMatchObject({ code: "unavailable" });
  });
  it("the readiness feed asks for a read again on a reset too, and on nothing else", async () => {
    const c = client(async () => stream(['{"type":"ready","bookmark":"x"}', '{"type":"reset"}', '{"type":"readiness_changed"}']));
    const onChange = vi.fn();
    await c.readinessLive(onChange);
    expect(onChange).toHaveBeenCalledTimes(2);
  });
});

describe("paged lists", () => {
  it("a page cursor the server refuses throws, keeps what was read, and may be asked again", async () => {
    let call = 0;
    const c = client(async (_u, init) => {
      const { input } = JSON.parse(String(init?.body)) as { input: { cursor?: string } };
      call++;
      if (call === 2) return json(400, { ok: false, error: { code: "invalid_input", message: "bad cursor" } });
      return json(200, { ok: true, data: { invites: [{ invite_id: `i${call}` }], next_cursor: input.cursor ? null : "c1" } });
    });
    const { server } = await import("./client");
    server.operate = c.operate.bind(c);
    const pg = people.invitePager("s");
    expect((await pg.next()).map((i) => i.invite_id)).toEqual(["i1"]);
    expect(pg.done).toBe(false);
    await expect(pg.next()).rejects.toMatchObject({ code: "invalid_input" });
    expect(pg.done).toBe(false); // not finished: the cursor is kept, so a retry asks the same page again
    expect((await pg.next()).map((i) => i.invite_id)).toEqual(["i3"]);
    expect(pg.done).toBe(true);
    expect(await pg.next()).toEqual([]); // past the end asks for nothing
    expect(call).toBe(3);
  });
});

describe("the device record", () => {
  it("is ignored when it is too old, damaged, or empty of spaces; and cleared on demand", () => {
    writeCache({ account: "a", capabilities: [], spaces: [{ id: "s", name: "Personal" }] });
    expect(readCache()?.account).toBe("a");
    localStorage.setItem("cal.boot", "{not json");
    expect(readCache()).toBeNull();
    writeCache({ account: "a", capabilities: [], spaces: [] });
    expect(readCache()).toBeNull();
    writeCache({ account: "a", capabilities: [], spaces: [{ id: "s", name: "P" }] });
    const old = JSON.parse(localStorage.getItem("cal.boot")!); old.at = Date.now() - 2 * 86_400_000; localStorage.setItem("cal.boot", JSON.stringify(old));
    expect(readCache()).toBeNull();
    writeCache({ account: "a", capabilities: [], spaces: [{ id: "s", name: "P" }] }); clearCache();
    expect(readCache()).toBeNull();
  });
});
