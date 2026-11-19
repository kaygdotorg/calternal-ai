// Tests for the second code review (N1 to N8 and finding 13). The theme of that review: store-level tests passed while the WIRING (session to AppView to stores) was wrong, so N3, N4 and N7 here drive the real
// HostSession with a fake transport and fake timers, and N4 mounts the real AppView.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { navHold } from "@calternal/ui";
import { MockHost } from "../testing/mock-host";
import { ACTIONS_KEY, ActionUi, AppView, ContentList, HostSession, Invoker, Selection, WindowStore } from "../src";
import { ImageCache } from "../src/render/images";
import type { AppDescription, Bind } from "../src";

const bind: Bind = { id: "notes", source: { type: "list", kind: "notes" }, window: { size: 60, count: "exact" }, row: { title: { field: "title" } } };
const settle = async () => { for (let i = 0; i < 25; i++) await vi.advanceTimersByTimeAsync(0); };
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); navHold.release(); });

async function listView(host: MockHost, o: { multiple?: boolean; inside?: boolean; b?: Bind } = {}) {
  const session = new HostSession({ transport: host }); await session.open("v");
  const store = new WindowStore({ bind: o.b ?? bind, source: session });
  const selection = new Selection("notes", session);
  const view = render(ContentList, { props: { store, selection, label: "n", multiple: o.multiple, inside: o.inside }, context: new Map<any, any>([[ACTIONS_KEY, { ui: new ActionUi(session), session }]]) });
  await vi.advanceTimersByTimeAsync(1500); await settle();
  return { session, store, selection, view };
}

describe("N1: the cursor survives two changes before the first reply", () => {
  it("two quick refreshes leave the selection settled again on the same row", async () => {
    const host = new MockHost({ rows: 5, actionIds: ["open"], latencyMs: 300 });
    const { store, selection, view } = await listView(host);
    await fireEvent.click(view.container.querySelector("[data-i] button.row")!); await settle();
    expect(selection.drawn).toBe("r-0");
    store.refresh(); await vi.advanceTimersByTimeAsync(20); // the first change clears the selection; its reply has not arrived yet
    store.refresh(); // the second finds the selection already cleared by the first
    await vi.advanceTimersByTimeAsync(3000); await settle();
    expect(selection.drawn).toBe("r-0"); // before the fix the cursor was lost for good
  });
});

describe("N2: a bulk selection survives the person's own action", () => {
  it("ticks stay through a re-read, an Undo-style change and a count-only change; only ids that left the loaded rows are dropped; a new query clears them", async () => {
    const host = new MockHost({ rows: 6, actionIds: ["open"] });
    const { store, selection } = await listView(host, { multiple: true });
    selection.toggle("r-1"); selection.toggle("r-3"); selection.toggle("r-4");
    host.version = "rv-2"; store.refresh(); await vi.advanceTimersByTimeAsync(2000); await settle();
    expect(selection.ticked).toEqual(["r-1", "r-3", "r-4"]); // same rows, new version
    host.rows = 7; host.version = "rv-3"; store.refresh(); await vi.advanceTimersByTimeAsync(2000); await settle();
    expect(selection.ticked).toEqual(["r-1", "r-3", "r-4"]); // only the count changed
    host.order = ["r-0", "r-2", "r-3", "r-4", "r-5", "r-6"]; host.rows = 6; host.version = "rv-4"; // r-1 was archived
    store.refresh(); await vi.advanceTimersByTimeAsync(2000); await settle();
    expect(selection.ticked).toEqual(["r-3", "r-4"]); // r-1 left; the others moved up and stay ticked
    host.version = "rv-5"; store.reset(); await vi.advanceTimersByTimeAsync(2000); await settle();
    expect(selection.ticked).toEqual([]); // a new query
  });
});

describe("N3: the lease lapse is reported on time, with the real session", () => {
  const lease = { lease_ref: "l-1", lease_seconds: 15 as const, grant_revision: "g", cache_partition: "cp" };
  it("a hung renewal still lapses at lease_seconds", async () => {
    const host = new MockHost({ rows: 1, renew: () => new Promise(() => {}) });
    const s = new HostSession({ transport: host }); const seen: boolean[] = []; s.oncurrent = (c) => seen.push(c);
    await s.open("v"); await vi.advanceTimersByTimeAsync(14_000);
    expect(s.current).toBe(true); expect(seen).toEqual([]);
    await vi.advanceTimersByTimeAsync(1_500);
    expect(s.current).toBe(false); expect(seen).toEqual([false]);
  });
  it("a renewal that fails fast does not move the lapse later", async () => {
    const host = new MockHost({ rows: 1, renew: () => { throw new Error("offline"); } });
    const s = new HostSession({ transport: host }); const at: number[] = []; s.oncurrent = (c) => { if (!c) at.push(Date.now()); };
    await s.open("v"); const t0 = Date.now(); await vi.advanceTimersByTimeAsync(20_000);
    expect(at.length).toBe(1); expect(at[0]! - t0).toBeLessThanOrEqual(15_100);
  });
  it("a reply that is not ours does not stop the renewals", async () => {
    let renews = 0;
    const host = new MockHost({ rows: 1, renew: () => { renews++; return { lease, retained_results: [], activation_contexts: {}, windows: [], renewal_id: "someone-else", request_lease_ref: "l-1", cache_partition: "cp" }; } });
    const s = new HostSession({ transport: host }); await s.open("v");
    await vi.advanceTimersByTimeAsync(40_000);
    expect(renews).toBeGreaterThanOrEqual(3); // before the fix the mismatch returned without scheduling, so there was exactly one
  });
});

describe("N4: a renewal does not un-hide windows it did not retain (the real AppView)", () => {
  it("both lists hide at the lapse; after a renewal only the retained one is shown, the other once its re-read arrives", async () => {
    const description: AppDescription = { version: 2, kind: "app", revision: 1,
      binds: [{ ...bind, id: "a" }, { ...bind, id: "b" }],
      chrome: { sidebar: { sections: [{ id: "sa", title: "A", bind: "a" }, { id: "sb", title: "B", bind: "b" }] } } };
    let fail = true; let hostRef: MockHost;
    const host: MockHost = hostRef = new MockHost({ rows: 3, latencyMs: 400, renew: (b) => {
      if (fail) throw new Error("offline");
      const w = hostRef.log.filter((l) => l.route === "window"); void w;
      return { lease: hostRef.lease, retained_results: [{ window_ref: "w-a-0", result_version: "rv-1" }], activation_contexts: {}, windows: [], renewal_id: b.renewal_id, request_lease_ref: b.lease_ref, cache_partition: b.cache_partition };
    } });
    const session = new HostSession({ transport: host }); await session.open("v");
    const view = render(AppView, { props: { description, session } });
    await vi.advanceTimersByTimeAsync(3000); await settle();
    const hidden = () => [...view.container.querySelectorAll<HTMLElement>(".list[data-bind]")].map((l) => `${l.dataset.bind}:${l.hasAttribute("data-hidden") ? "hidden" : "shown"}`).join(",");
    expect(hidden()).toBe("a:shown,b:shown");
    await vi.advanceTimersByTimeAsync(16_000); // renewals fail: the lease lapses
    expect(hidden()).toBe("a:hidden,b:hidden");
    fail = false;
    for (let t = 0; t < 12_000 && hidden().startsWith("a:hidden"); t += 50) await vi.advanceTimersByTimeAsync(50); // until the next renewal succeeds and names only a's window
    expect(hidden()).toBe("a:shown,b:hidden"); // b stays hidden until its own reply (before the fix every store was un-hidden at once)
    await vi.advanceTimersByTimeAsync(2000); await settle();
    expect(hidden()).toBe("a:shown,b:shown");
  });
});

describe("N5: an id that moved across chunks keeps its index and its context", () => {
  it("indexOf and contexts survive a later chunk overwriting the old position", async () => {
    const host = new MockHost({ rows: 120, actionIds: ["open"] });
    host.order = Array.from({ length: 120 }, (_, i) => `x-${i}`);
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind, source: session });
    store.ensure("a", 0, 90); await vi.advanceTimersByTimeAsync(3000); await settle();
    expect(store.indexOf("x-60")).toBe(60);
    host.order = host.order.filter((id) => id !== "x-10"); host.rows = 119; host.version = "rv-2"; // everything after x-10 moves up by one
    store.refresh(); await vi.advanceTimersByTimeAsync(3000); await settle();
    expect(store.indexOf("x-60")).toBe(59); expect(store.contexts("x-60")?.open).toBeTruthy(); // before the fix the second chunk's overwrite deleted its entry
    expect(store.indexOf("x-10")).toBe(-1);
  });
});

describe("N6: a cancelled read is not joined by a row that comes back", () => {
  const ref = (n: number) => `ref${String(n).padStart(20, "0")}`;
  beforeEach(() => { (URL as any).createObjectURL = () => "blob:x"; (URL as any).revokeObjectURL = () => {}; });
  it("a read queued behind the cap and then abandoned is replaced, not shared", async () => {
    const gates: (() => void)[] = []; let reads = 0;
    const c = new ImageCache(() => { reads++; return new Promise((res) => { gates.push(() => res(new Blob(["x"]))); }); }, { concurrency: 1 });
    const first = c.get(ref(1)); await settle();
    const ctl = new AbortController(); const queued = c.get(ref(2), ctl.signal); await settle();
    ctl.abort(); expect(await queued).toBeNull();
    const again = c.get(ref(2)); // the row scrolled back while ref 1 still holds the only slot
    gates.shift()!(); await settle(); await first; gates.shift()?.(); await settle();
    expect(await again).toBe("blob:x"); // before the fix it joined the aborted entry and got null for good
    expect(reads).toBe(2);
  });
});

describe("N7: an activation on a lapsed view is refused before it is sent", () => {
  it("the real session lapses, the invoker sends nothing and says why", async () => {
    const host = new MockHost({ rows: 1, renew: () => { throw new Error("offline"); }, handle: () => ({ activation_id: "a", state: "ok" }) });
    const session = new HostSession({ transport: host }); await session.open("v");
    const seen: string[] = [];
    const inv = new Invoker(session, { prompt: async () => "x", choose: async () => null, confirm: async () => true, outcome: (_a, o) => seen.push(`${o.state}:${o.code}`) });
    await vi.advanceTimersByTimeAsync(16_000);
    expect(session.current).toBe(false);
    const out = await inv.run({ id: "a", operation: "host.item.patch-metadata" }, { context: "ctx" });
    expect(out).toMatchObject({ state: "error", code: "lease_lapsed" });
    expect(host.log.filter((l) => l.route.startsWith("activation"))).toEqual([]); // nothing was sent
    expect(seen).toEqual(["error:lease_lapsed"]);
  });
});

describe("N8: image reads wait for a key burst to settle; a miss is remembered", () => {
  const ref = (n: number) => `ref${String(n).padStart(20, "0")}`;
  beforeEach(() => { (URL as any).createObjectURL = () => "blob:x"; (URL as any).revokeObjectURL = () => {}; });
  it("does not start a read while a key repeats, cancels one in flight when a burst begins, and goes on after the rest", async () => {
    let held = false; const rel = new Set<() => void>(), beg = new Set<() => void>();
    const hold = { get held() { return held; }, subscribe: (f: () => void) => { rel.add(f); return () => rel.delete(f); }, subscribeBegin: (f: () => void) => { beg.add(f); return () => beg.delete(f); } };
    const started: string[] = []; const signals: AbortSignal[] = [];
    const c = new ImageCache((r, sig) => { started.push(r); signals.push(sig!); return new Promise((res, rej) => { sig!.addEventListener("abort", () => rej(new Error("aborted"))); setTimeout(() => res(new Blob(["x"])), 100); }); }, { hold });
    held = true; const p1 = c.get(ref(1)); await settle();
    expect(started).toEqual([]); // deferred while a key repeats
    held = false; rel.forEach((f) => f()); await vi.advanceTimersByTimeAsync(10);
    expect(started).toEqual([ref(1)]);
    held = true; beg.forEach((f) => f()); await settle(); // a burst begins while it is in flight
    expect(signals[0]!.aborted).toBe(true);
    held = false; rel.forEach((f) => f()); await vi.advanceTimersByTimeAsync(300); await settle();
    expect(started).toEqual([ref(1), ref(1)]); // read again once it settled
    expect(await p1).toBe("blob:x");
  });
  it("remembers a miss, so a row showing its miniature does not ask again on every mount", async () => {
    let reads = 0; const c = new ImageCache(async () => { reads++; return null; });
    expect(await c.get(ref(9))).toBeNull(); expect(await c.get(ref(9))).toBeNull(); expect(await c.get(ref(9))).toBeNull();
    expect(reads).toBe(1);
  });
});

describe("finding 13: the keyboard reaches a row's controls without the context-menu key", () => {
  const withActions: Bind = { ...bind, row: { title: { field: "title" }, trailingActions: [{ id: "t", title: "Trash", icon: "trash", operation: "host.item.trash", inputs: {} }], menu: [{ id: "r", title: "Rename", operation: "host.item.rename", inputs: {} }] } };
  it("ArrowRight from the row moves to the tick box (named by the row title), then the actions; ArrowLeft and Escape come back", async () => {
    const host = new MockHost({ rows: 3, actionIds: ["t", "r"], fields: (i) => ({ title: `Note number ${i}` }) });
    const { view } = await listView(host, { multiple: true, b: withActions });
    const row = view.container.querySelector<HTMLElement>("[data-i='0']")!, main = row.querySelector<HTMLElement>("button.row")!;
    const box = row.querySelector<HTMLInputElement>("label.tick input")!;
    expect(box.getAttribute("aria-label")).toBe("Note number 0"); // the row's title, not its opaque id
    main.focus(); await fireEvent.keyDown(main, { key: "ArrowRight" });
    expect(document.activeElement).toBe(box);
    await fireEvent.keyDown(box, { key: "ArrowRight" });
    expect((document.activeElement as HTMLElement).dataset.action).toBe("t");
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowRight" });
    expect((document.activeElement as HTMLElement).classList.contains("more")).toBe(true);
    await fireEvent.keyDown(document.activeElement!, { key: "ArrowLeft" });
    expect((document.activeElement as HTMLElement).dataset.action).toBe("t");
    await fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(document.activeElement).toBe(main);
  });
  it("without `inside` no control of a row is a Tab stop (the list's scroller is the one stop)", async () => {
    const host = new MockHost({ rows: 4, actionIds: ["t", "r"] });
    const { view } = await listView(host, { multiple: true, b: withActions });
    const inRows = [...view.container.querySelectorAll<HTMLElement>("[data-i] button, [data-i] input")].filter((e) => e.tabIndex >= 0);
    expect(view.container.querySelectorAll("[data-i]").length).toBe(4);
    expect(inRows.length).toBe(0);
  });
});
