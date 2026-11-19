import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { navHold } from "@calternal/ui";
import { MockHost } from "../testing/mock-host";
import { HostSession, WindowStore } from "../src";
import type { Bind } from "../src";

const bind: Bind = { id: "notes", source: { type: "list", kind: "notes" }, window: { size: 60, count: "exact" }, row: { title: { field: "title" } } };
const settle = async () => { for (let i = 0; i < 20; i++) await vi.advanceTimersByTimeAsync(0); };

async function open(host: MockHost, b: Bind = bind) {
  const session = new HostSession({ transport: host });
  await session.open("v");
  const store = new WindowStore({ bind: b, source: session });
  return { session, store };
}

beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); navHold.release(); });

describe("WindowStore", () => {
  it("asks for the first window before the list has measured itself, and shows loading until it arrives", async () => {
    const host = new MockHost({ rows: 5000 });
    const { store } = await open(host);
    store.ensure("a", 0, 0);
    expect(store.state).toBe("loading");
    await settle();
    expect(store.state).toBe("ready");
    expect(store.length).toBe(5000);
    expect(store.row(0)?.fields.title).toBe("Row 0");
    expect(store.row(59)).toBeDefined();
  });

  it("loads windows around what is drawn, with a margin, and nothing far away", async () => {
    const host = new MockHost({ rows: 5000 });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    store.ensure("a", 2000, 2030); await vi.advanceTimersByTimeAsync(1100); await settle();
    expect(store.row(2000)).toBeDefined(); expect(store.row(2029)).toBeDefined();
    expect(store.row(1000)).toBeUndefined(); expect(store.row(4000)).toBeUndefined();
    for (const r of host.log.filter((l) => l.route === "window")) { const q = (r.body as { request: { from: number; to: number } }).request; expect(q.to - q.from).toBeLessThanOrEqual(200); }
  });

  it("never sends more than 4 window requests a second", async () => {
    const times: number[] = [];
    const host = new MockHost({ rows: 5000, onWindow: () => times.push(Date.now()) });
    const { store } = await open(host);
    for (let k = 0; k < 20; k++) { store.ensure("a", k * 240, k * 240 + 30); await vi.advanceTimersByTimeAsync(10); }
    await vi.advanceTimersByTimeAsync(5000); await settle();
    expect(times.length).toBeGreaterThan(4);
    for (let i = 0; i + 4 < times.length; i++) expect(times[i + 4]! - times[i]!).toBeGreaterThanOrEqual(1000);
  });

  it("an exact-count empty reply at from 0 is the empty state", async () => {
    const { store } = await open(new MockHost({ rows: 0 }));
    store.ensure("a", 0, 0); await settle();
    expect(store.state).toBe("empty"); expect(store.length).toBe(0);
  });

  it("a later-page `empty` is refused: the rows already shown stay", async () => {
    const host = new MockHost({ rows: 120, intercept: (req) => (req.from >= 60 ? { audience: "own_space_member", bind: "notes", state: "empty", result_version: "rv-1", offset: 0, rows: [], window_ref: "w", count: { mode: "exact", value: 0 } } as never : undefined) });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    store.ensure("a", 50, 80); await vi.advanceTimersByTimeAsync(1500); await settle();
    expect(store.state).toBe("ready"); expect(store.row(0)).toBeDefined();
  });

  it("without a count the list grows by one sentinel row until a short ready window ends it", async () => {
    const host = new MockHost({ rows: 100, count: "none" });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    expect(store.exact).toBe(false);
    expect(store.length).toBe(61); // 60 loaded + the sentinel: no total is implied
    store.ensure("a", 40, 61); await vi.advanceTimersByTimeAsync(1100); await settle();
    expect(store.length).toBe(100); // the second window was short: that is the end
    expect(store.row(99)).toBeDefined();
  });

  it("a new result_version replaces the window wholesale; the same version only merges", async () => {
    const host = new MockHost({ rows: 200 });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    host.version = "rv-2"; host.rows = 10;
    store.ensure("a", 0, 40); store.ensure("a", 100, 130); await vi.advanceTimersByTimeAsync(1100); await settle();
    expect(store.resultVersion).toBe("rv-2"); expect(store.length).toBe(10);
    expect(store.row(100)).toBeUndefined();
  });

  it("holds a version-changing reply while a key repeats and applies it once the selection settles", async () => {
    const host = new MockHost({ rows: 200 });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    host.version = "rv-2"; host.rows = 7;
    navHold.begin();
    store.ensure("a", 50, 80); await vi.advanceTimersByTimeAsync(300); await settle(); // inside the hold's 500 ms maximum
    expect(store.resultVersion).toBe("rv-1"); expect(store.length).toBe(200); // drawn version unchanged during the burst
    navHold.release(); await settle();
    expect(store.resultVersion).toBe("rv-2"); expect(store.length).toBe(7);
  });

  it("applies at once a reply for the version already drawn, even while a key repeats (arrowing never meets a skeleton)", async () => {
    const host = new MockHost({ rows: 500 });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    navHold.begin();
    store.ensure("a", 100, 130); await vi.advanceTimersByTimeAsync(1100); await settle();
    expect(store.row(100)).toBeDefined();
  });

  it("an end-anchored reply places its rows by their distance from the end", async () => {
    const host: MockHost = new MockHost({ rows: 1000, intercept: (req) => (req.from === 0 ? host.answer({ bind: "notes", from: 0, to: 60, anchor: "end" }) : undefined) });
    const { store } = await open(host);
    store.ensure("a", 0, 0); await settle();
    expect(store.row(999)?.id).toBe("r-999"); expect(store.row(940)?.id).toBe("r-940");
    expect(store.row(939)).toBeUndefined(); expect(store.row(0)).toBeUndefined();
  });

  it("does not request after destroy and ignores late replies", async () => {
    const host = new MockHost({ rows: 100, latencyMs: 50 });
    const { store } = await open(host);
    store.ensure("a", 0, 0); store.destroy(); await vi.advanceTimersByTimeAsync(200);
    expect(store.row(0)).toBeUndefined();
  });
});
