import { describe, expect, it } from "vitest";
import { HostError, HostSession, ProductTransport } from "../src";

function fake() {
  const calls: { url: string; method: string; headers: Record<string, string>; body?: unknown }[] = [];
  const f = (async (url: string, init: RequestInit = {}) => {
    calls.push({ url, method: init.method ?? "GET", headers: (init.headers ?? {}) as Record<string, string>, body: init.body ? JSON.parse(String(init.body)) : undefined });
    const u = String(url);
    const json = (o: unknown, status = 200) => new Response(JSON.stringify(o), { status });
    if (u.endsWith("/window")) return calls.at(-1)!.body && (calls.at(-1)!.body as any).request.bind === "reminders" ? json({ status: 422, code: "unsupported_query_plan", reason: "unsupported_plan" }, 422) : json({ bind: "items", state: "ready", result_version: "v", offset: 0, rows: [], window_ref: "w", audience: "own_space_member", count: { mode: "exact", value: 0 } });
    if (u.endsWith("/reading-list-view") && !init.method) return json({ view_ref: "vr", description: { version: 2, kind: "app", revision: 1 } });
    if (u.endsWith("/reading-list-view")) return json({ lease: { lease_ref: "l1", lease_seconds: 15, grant_revision: "g", cache_partition: "cp1" }, activation_contexts: {}, audience: "own_space_member" });
    return json({ code: "nope", message: "no" }, 404);
  }) as unknown as typeof fetch;
  return { calls, f };
}

describe("ProductTransport", () => {
  it("describes, opens, and sends the lease and cache partition on window requests", async () => {
    const { calls, f } = fake();
    const t = new ProductTransport({ base: "http://h/api/v2/spaces/s/reading-list-view/", bearer: "tok", fetch: f });
    expect((await t.describe()).view_ref).toBe("vr");
    const s = new HostSession({ transport: t });
    await s.open("vr");
    await s.window({ bind: "items", from: 0, to: 60 });
    const w = calls.at(-1)!;
    expect(w.url).toBe("http://h/api/v2/spaces/s/reading-list-view/window");
    expect(w.headers["x-calternal-view-lease"]).toBe("l1"); expect(w.headers["x-calternal-cache-partition"]).toBe("cp1");
    expect(w.headers.authorization).toBe("Bearer tok");
    expect(w.body).toEqual({ type: "window", request: { bind: "items", from: 0, to: 60 } });
  });
  it("hands back the sealed 422 refusal, and fails routes the slice does not serve with 501", async () => {
    const { f } = fake();
    const t = new ProductTransport({ base: "http://h/api/reading-list-view", bearer: "t", fetch: f });
    const s = new HostSession({ transport: t }); await s.open("vr");
    expect(await s.window({ bind: "reminders", from: 0, to: 20 })).toMatchObject({ status: 422, reason: "unsupported_plan" });
    await expect(t.post("activation.resolve", {})).rejects.toBeInstanceOf(HostError);
  });
});
