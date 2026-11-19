import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { navHold } from "@calternal/ui";
import { MockHost } from "../testing/mock-host";
import { HostSession, WindowStore } from "../src";
import type { Bind, WindowRequestPayload } from "../src";

const bind: Bind = { id: "notes", source: { type: "list", kind: "notes" }, window: { size: 60, count: "exact" }, row: { title: { field: "title" } } };
const run = async (ms: number) => { for (let t = 0; t < ms; t += 250) { await vi.advanceTimersByTimeAsync(250); } for (let i = 0; i < 20; i++) await vi.advanceTimersByTimeAsync(0); };
async function open(host: MockHost, b: Bind = bind) {
  const session = new HostSession({ transport: host }); await session.open("v");
  const store = new WindowStore({ bind: b, source: session });
  store.ensure("a", 0, 0); await run(300);
  return store;
}
const reqs = (host: MockHost) => host.log.filter((l) => l.route === "window").map((l) => (l.body as { request: WindowRequestPayload }).request);
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); navHold.release(); });

describe("typed refusals (docs/description-contract.md 2.3, recovery rule)", () => {
  it("skip_limit near the start: pages forward from the start toward the retained target", async () => {
    const host = new MockHost({ rows: 100_000, skipLimit: 500 });
    const store = await open(host);
    store.ensure("a", 1500, 1530); await run(12000);
    expect(reqs(host).some((r) => r.from === 1500 && !r.anchor)).toBe(true); // the jump was tried first and refused
    expect(store.row(1500)).toBeDefined(); expect(store.state).toBe("ready");
    const walk = reqs(host).filter((r) => r.from > 60 && r.from < 1500 && !r.anchor);
    expect(walk.length).toBeGreaterThan(5);
    for (const r of walk) expect(r.from % 60).toBe(0); // contiguous windows, no gaps
  });

  it("skip_limit in the far half: pages backward from the end (end-anchored, contiguous distances)", async () => {
    const host = new MockHost({ rows: 5000, skipLimit: 500 });
    const store = await open(host);
    store.ensure("a", 3500, 3530); await run(15000);
    // 3500 is 1500 rows from the end; the tail walk is end-anchored with growing distance
    const ends = reqs(host).filter((r) => r.anchor === "end");
    expect(ends.length).toBeGreaterThan(5);
    for (let i = 1; i < ends.length; i++) expect(ends[i]!.from).toBeGreaterThanOrEqual(ends[i - 1]!.from);
    expect(store.row(3500)).toBeDefined(); expect(store.row(3500)!.id).toBe("r-3500");
  });

  it("end_unavailable: pages forward from the last loaded row, and End on a list without exact count walks to its end", async () => {
    const host = new MockHost({ rows: 300, count: "none" });
    const store = await open(host);
    expect(store.exact).toBe(false);
    store.jump("end"); await run(8000);
    expect(store.length).toBe(300); expect(store.row(299)?.id).toBe("r-299"); expect(store.walkGoal).toBeNull();
    expect(reqs(host).every((r) => r.anchor !== "end")).toBe(true);
  });

  it("end_unavailable for an exact list: the tail request is refused, then it pages forward and the target is kept", async () => {
    const host = new MockHost({ rows: 400, endUnavailable: true });
    const store = await open(host);
    store.ensure("a", 380, 400); await run(8000);
    expect(store.row(399)?.id).toBe("r-399");
    expect(reqs(host).filter((r) => r.anchor === "end").length).toBeLessThanOrEqual(2); // only the requests already in flight when the first refusal came; never again after
  });

  it("refine_query stops forward paging and keeps the rows shown", async () => {
    let n = 0;
    const host: MockHost = new MockHost({ rows: 5000, count: "none", intercept: (req) => (++n > 3 ? ({ audience: "own_space_member", bind: "notes", state: "refine_query", result_version: null, offset: req.from, rows: [{ id: "x", fields: { title: "x" } }], window_ref: null } as never) : undefined) });
    const store = await open(host);
    store.jump("end"); await run(6000);
    const sent = reqs(host).length;
    await run(6000);
    expect(store.state).toBe("refine_query"); expect(reqs(host).length).toBe(sent); expect(store.walkGoal).toBeNull(); expect(store.row(0)).toBeDefined();
  });

  it("any other reason shows the bind's error state, and a refused recovery request does too", async () => {
    for (const reason of ["unsupported_plan", "materialization_limit"]) {
      const host = new MockHost({ rows: 100, intercept: () => ({ status: 422, code: "unsupported_query_plan", reason }) as never });
      const session = new HostSession({ transport: host }); await session.open("v");
      const store = new WindowStore({ bind, source: session }); store.ensure("a", 0, 0); await run(300);
      expect(store.state).toBe("error"); expect(store.planError).toBe(reason);
    }
  });
});
