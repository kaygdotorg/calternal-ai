// Tests for the findings of the independent review of 6e989eab: each pins a behaviour that was wrong before its fix.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { navHold } from "@calternal/ui";
import { MockHost } from "../testing/mock-host";
import { ACTIONS_KEY, APP_KEY, ActionUi, ContentList, HostError, HostSession, HttpTransport, Invoker, Selection, WindowStore } from "../src";
import { MAX_ROWS } from "../src/windows/window-store.svelte";
import { failure } from "../src/invoke/invoker";
import { shellAllowed } from "../src/invoke/shell";
import { ImageCache } from "../src/render/images";
import type { Action, ActionOutcome, Bind, Invoke, WindowReply, WindowRequestPayload } from "../src";

const bind: Bind = { id: "notes", source: { type: "list", kind: "notes" }, window: { size: 60, count: "exact" }, row: { title: { field: "title" } } };
const settle = async () => { for (let i = 0; i < 25; i++) await vi.advanceTimersByTimeAsync(0); };
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); navHold.release(); });

/** A source whose rows carry the query that was current when the REQUEST was made, answered later. */
function lateSource(mode: { q: string }, delay = 100, rows = 500) {
  const signals: AbortSignal[] = []; let version = 0;
  return { signals, source: { window(req: WindowRequestPayload, signal?: AbortSignal): Promise<WindowReply> {
    const q = mode.q; if (signal) signals.push(signal); const v = ++version;
    return new Promise((res) => setTimeout(() => res({ audience: "own_space_member", bind: req.bind, state: "ready", result_version: `rv-${q}`, offset: req.from, window_ref: `w${v}`, count: { mode: "exact", value: rows },
      rows: Array.from({ length: Math.min(rows - req.from, req.to - req.from) }, (_, k) => ({ id: `${q}-${req.from + k}`, fields: { title: `${q} ${req.from + k}` } })) } as unknown as WindowReply), delay));
  } } };
}

describe("finding 1: reset drops what is in flight", () => {
  it("an answer to the old query that arrives after reset() is dropped and its request aborted", async () => {
    const mode = { q: "A" }; const s = lateSource(mode);
    const store = new WindowStore({ bind, source: s.source });
    store.ensure("x", 0, 30);
    await vi.advanceTimersByTimeAsync(10); // the A request is in flight
    mode.q = "B"; store.reset();
    await vi.advanceTimersByTimeAsync(400); await settle();
    expect(s.signals[0]!.aborted).toBe(true);
    expect(store.row(0)?.fields.title).toBe("B 0"); // not the stale "A 0"
    expect(store.resultVersion).toBe("rv-B");
  });
});

describe("finding 2: a re-read replaces the whole window", () => {
  it("rows far from view loaded under the old result set are not kept after refresh()", async () => {
    const mode = { q: "old" }; const s = lateSource(mode, 10);
    const store = new WindowStore({ bind, source: s.source });
    store.ensure("x", 0, 30); await vi.advanceTimersByTimeAsync(1200); await settle();
    store.ensure("x", 240, 270); await vi.advanceTimersByTimeAsync(1500); await settle();
    expect(store.row(240)?.fields.title).toBe("old 240");
    store.ensure("x", 0, 30); await vi.advanceTimersByTimeAsync(300);
    mode.q = "new"; store.refresh(); await vi.advanceTimersByTimeAsync(1500); await settle();
    store.ensure("x", 240, 270); await vi.advanceTimersByTimeAsync(2500); await settle();
    expect(store.row(240)?.fields.title).toBe("new 240"); // not the stale "old 240"
    expect(store.row(0)?.fields.title).toBe("new 0");
  });
});

describe("finding 3: the selection follows the list", () => {
  it("reset() and refresh() bump generation; the cursor row is settled again so its contexts are fresh, and a vanished row clears the selection", async () => {
    const host = new MockHost({ rows: 5, actionIds: ["open"] });
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind, source: session });
    const selection = new Selection("notes", session);
    const view = render(ContentList, { props: { store, selection, label: "n" }, context: new Map<any, any>([[ACTIONS_KEY, { ui: new ActionUi(session), session, viewContext: (id: string) => session.viewContexts[id] }]]) });
    await settle();
    await fireEvent.click(view.container.querySelector("[data-i] button.row")!); await settle();
    expect(selection.drawn).toBe("r-0"); expect(selection.sent).toBe(1);
    const g = store.generation;
    store.refresh(); await vi.advanceTimersByTimeAsync(1500); await settle();
    expect(store.generation).toBeGreaterThan(g);
    expect(selection.sent).toBe(2); expect(selection.drawn).toBe("r-0"); // settled again: fresh context, not the pre-mutation one
    selection.toggle("r-1"); expect(selection.ticked).toEqual(["r-1"]);
    host.rows = 0; host.version = "rv-2"; store.reset(); await vi.advanceTimersByTimeAsync(1500); await settle();
    expect(selection.drawn).toBeNull(); expect(selection.ticked).toEqual([]); // the row is gone: nothing to act on
  });
});

describe("finding 4: the lease lapse is enforced", () => {
  it("hidden rows offer no contexts; a renewal that names the window repaints it, otherwise it is read again", async () => {
    const host = new MockHost({ rows: 5, actionIds: ["open"] });
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind, source: session });
    store.ensure("x", 0, 5); await vi.advanceTimersByTimeAsync(1200); await settle();
    expect(store.contexts("r-0")?.open).toBeTruthy();
    store.setCurrent(false);
    expect(store.hidden).toBe(true); expect(store.contexts("r-0")).toBeUndefined();
    store.renewed([{ window_ref: store.windowRef!, result_version: store.resultVersion! }]);
    expect(store.hidden).toBe(false); expect(store.contexts("r-0")?.open).toBeTruthy();
    store.setCurrent(false); const calls = host.windowCalls;
    store.renewed([]); await vi.advanceTimersByTimeAsync(1500); await settle();
    expect(host.windowCalls).toBeGreaterThan(calls); expect(store.hidden).toBe(false); // read again, shown after its reply
  });
  it("rows dropped from the store take their contexts with them", async () => {
    const host = new MockHost({ rows: 600, actionIds: ["open"] });
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind, source: session });
    store.ensure("x", 0, 30); await vi.advanceTimersByTimeAsync(1200); await settle();
    store.ensure("x", 300, 330); await vi.advanceTimersByTimeAsync(2500); await settle();
    store.refresh(); // drops what is far from the drawn range, contexts included
    expect(store.contexts("r-0")).toBeUndefined();
  });
});

describe("finding 5: no renewal after close", () => {
  it("a renewal still in flight when the view closes starts no timer", async () => {
    let release!: () => void; let renews = 0;
    const host = new MockHost({ rows: 1, handle: undefined });
    const orig = host.post.bind(host);
    host.post = (async (route: any, body: any) => { if (route === "view.renew") { renews++; await new Promise<void>((r) => { release = r; }); } return orig(route, body); }) as never;
    const session = new HostSession({ transport: host }); await session.open("v");
    await vi.advanceTimersByTimeAsync(10_000); // the renewal starts
    expect(renews).toBe(1);
    session.close(); release();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(renews).toBe(1); // before the fix the late answer rescheduled the timer
  });
});

describe("findings 6 and 7: row images", () => {
  const ref = (n: number) => `ref${String(n).padStart(20, "0")}`;
  let made = 0, revoked = 0;
  beforeEach(() => { made = 0; revoked = 0; (URL as any).createObjectURL = () => `blob:${++made}`; (URL as any).revokeObjectURL = () => { revoked++; }; });
  it("shares one read per ref, keeps a bounded cache and revokes what it evicts", async () => {
    let reads = 0; const c = new ImageCache(async () => { reads++; return new Blob(["x"]); }, { max: 3 });
    await Promise.all([c.get(ref(1)), c.get(ref(1))]); expect(reads).toBe(1);
    for (let i = 2; i <= 6; i++) await c.get(ref(i));
    expect(c.size).toBe(3); expect(revoked).toBe(3);
    await c.get(ref(6)); expect(reads).toBe(6); // a hit costs no read
  });
  it("reads at most 4 at once and cancels a read when the last row that wanted it leaves", async () => {
    let active = 0, peak = 0; const gates: (() => void)[] = []; const signals: AbortSignal[] = [];
    const c = new ImageCache((_r, sig) => { active++; peak = Math.max(peak, active); signals.push(sig!); return new Promise((res) => { gates.push(() => { active--; res(new Blob(["x"])); }); }); }, { concurrency: 4 });
    const ctl = new AbortController();
    const ps = Array.from({ length: 10 }, (_, i) => c.get(ref(i), i === 0 ? ctl.signal : undefined));
    await settle(); expect(peak).toBe(4);
    ctl.abort(); expect(signals[0]!.aborted).toBe(true); expect(await ps[0]).toBeNull();
    for (let i = 0; i < 12; i++) { gates.shift()?.(); await settle(); }
    expect(peak).toBe(4);
  });
  it("never fetches a ref that does not have the host's shape (a `..` would climb out of the route with the bearer)", async () => {
    let reads = 0; const c = new ImageCache(async () => { reads++; return new Blob(["x"]); });
    for (const bad of ["..", "../../x", "a/b", "short", "x".repeat(200), "has space ".repeat(3)]) expect(await c.get(bad)).toBeNull();
    expect(reads).toBe(0);
    let fetched = 0; const t = new HttpTransport({ base: "http://h/step0/v2", bearer: "b", fetch: (async () => { fetched++; return new Response("x"); }) as never });
    expect(await t.image("..")).toBeNull(); expect(fetched).toBe(0);
  });
  it("refuses an oversized image by its declared length before reading the body", async () => {
    const getReader = vi.fn(), cancel = vi.fn(async () => {});
    const big = { ok: true, status: 200, headers: new Headers({ "content-type": "image/png", "content-length": "70000" }), body: { getReader, cancel } };
    const t = new HttpTransport({ base: "http://h/step0/v2", bearer: "b", fetch: (async () => big) as never });
    expect(await t.image(ref(1))).toBeNull(); expect(getReader).not.toHaveBeenCalled(); expect(cancel).toHaveBeenCalled();
    const t2 = new HttpTransport({ base: "http://h/step0/v2", bearer: "b", fetch: (async () => new Response(new Uint8Array(70000), { headers: { "content-type": "image/png" } })) as never });
    expect(await t2.image(ref(2))).toBeNull(); // no declared length: reading stops at the cap
  });
});

describe("finding 9: what a failure says", () => {
  const act: Invoke = { id: "a", operation: "reading.add" };
  it("the host's own refusal is shown with the host's words; a failure before anything ran says nothing happened; only a lost answer after a request that may have run it is unknown", () => {
    expect(failure(new HostError(403, "forbidden", "You may not do that."), false)).toMatchObject({ state: "error", message: "You may not do that." });
    expect(failure(new HostError(409, "stale", "The note changed."), true)).toMatchObject({ state: "stale", message: "The note changed." });
    expect(failure(new TypeError("fetch failed"), false)).toMatchObject({ state: "error", code: "not_started" });
    expect(failure(new TypeError("fetch failed"), true)).toMatchObject({ state: "unknown" });
    expect(failure(new HostError(500, undefined, "boom"), true)).toMatchObject({ state: "unknown" });
    void act;
  });
  it("a failing choice fetch is an error, not unknown, and a still-pending outcome is returned as pending", async () => {
    const host = new MockHost({ rows: 1, handle: (route) => { if (route === "choice.options") throw new HostError(404, "not_found", "No such action."); if (route === "status") return { activation_id: "a", state: "pending", status_ref: "s" }; return { activation_id: "a", state: "pending", status_ref: "s" }; } });
    const session = new HostSession({ transport: host }); await session.open("v");
    const seen: string[] = [];
    const inv = new Invoker(session, { prompt: async () => "x", choose: async () => "c", confirm: async () => true, outcome: (_a, o) => seen.push(`${o.state}:${o.message ?? ""}`) });
    const move: Invoke = { id: "m", operation: "host.collection.move", inputs: { collection: { prompt: { title: "t", kind: "choice", options: { bind: "c" } } } } };
    expect(await inv.run(move, { context: "c" })).toMatchObject({ state: "error", message: "No such action." });
    const p = inv.run({ id: "p", operation: "reading.add" }, { context: "c2" });
    await vi.advanceTimersByTimeAsync(200_000);
    expect(await p).toMatchObject({ state: "pending" });
  });
});

describe("findings 10 and 11: dialogs", () => {
  it("the confirmation title is the host's, never the plugin's action title", async () => {
    const ui = new ActionUi({} as never);
    void ui.confirm({ action: { id: "x", title: "Totally harmless", operation: "o" }, material: [] });
    expect(ui.dialog).toMatchObject({ kind: "confirm", title: "Please confirm" });
  });
  it("opening a dialog settles the one it replaces, so no activation is left waiting for ever", async () => {
    const ui = new ActionUi({} as never);
    const first = ui.prompt({ name: "a", title: "A", kind: "text" });
    const second = ui.confirm({ action: { id: "x", operation: "o" }, material: [] });
    expect(await first).toBeNull();
    expect(ui.dialog?.kind).toBe("confirm");
    ui.dialog!.done(false as never); expect(await second).toBe(false);
  });
});

describe("finding 12: shell actions", () => {
  it("only the shell controls the contract allows, well formed, reach the shell", () => {
    const a = (shell: unknown) => ({ id: "s", shell } as unknown as Action);
    expect(shellAllowed(a({ signOut: true }))).toBe(true);
    expect(shellAllowed(a({ sheet: "trash" }))).toBe(true);
    expect(shellAllowed(a({ sheet: "evil" }))).toBe(false);
    expect(shellAllowed(a({ route: { name: "x", value: { literal: "y" } }, reload: true }))).toBe(false);
    expect(shellAllowed(a({ eval: "alert(1)" }))).toBe(false);
    expect(shellAllowed({ id: "i", operation: "x" } as Action)).toBe(false);
  });
});

describe("finding 13: one Tab stop per list", () => {
  it("with `inside` exactly one control of the list is tabbable; the controls inside rows never are", async () => {
    const withActions: Bind = { ...bind, row: { title: { field: "title" }, trailingActions: [{ id: "t", title: "Trash", operation: "host.item.trash", inputs: {} }], menu: [{ id: "r", title: "Rename", operation: "host.item.rename", inputs: {} }] } };
    const host = new MockHost({ rows: 6, actionIds: ["t", "r"] });
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind: withActions, source: session });
    const scroller = document.createElement("div"); scroller.style.overflowY = "auto"; document.body.append(scroller); // the scrolling ancestor an `inside` list follows
    const view = render(ContentList, { target: scroller, props: { store, selection: new Selection("notes", session), label: "n", inside: true, multiple: true }, context: new Map<any, any>([[ACTIONS_KEY, { ui: new ActionUi(session), session, viewContext: (id: string) => session.viewContexts[id] }]]) });
    await vi.advanceTimersByTimeAsync(1500); await settle(); await settle();
    const tabbable = [...view.container.querySelectorAll<HTMLElement>("button, input, a, select, textarea")].filter((e) => e.tabIndex >= 0);
    expect(view.container.querySelectorAll("[data-i]").length).toBe(6);
    expect(tabbable.length).toBe(1);
  });
});

describe("finding 14: a stated row bound and an id index", () => {
  it("indexOf is exact after rows are dropped, and a long scroll holds at most MAX_ROWS rows", async () => {
    const host = new MockHost({ rows: 20_000 });
    const session = new HostSession({ transport: host }); await session.open("v");
    const store = new WindowStore({ bind, source: session });
    for (let i = 0; i < 60; i++) { store.ensure("x", i * 60, i * 60 + 30); await vi.advanceTimersByTimeAsync(300); await settle(); }
    await vi.advanceTimersByTimeAsync(3000); await settle();
    let held = 0; for (let i = 0; i < 20_000; i++) if (store.row(i)) held++;
    expect(held).toBeLessThanOrEqual(MAX_ROWS + 60);
    for (let i = 0; i < 20_000; i++) { const r = store.row(i); if (r) { expect(store.indexOf(r.id)).toBe(i); break; } }
    expect(store.indexOf("r-0")).toBe(store.row(0) ? 0 : -1);
  });
});

describe("dialogs close when answered", () => {
  it("answering the open dialog clears it, so its scrim goes", async () => {
    const ui = new ActionUi({} as never);
    const p = ui.prompt({ name: "a", title: "A", kind: "text" });
    expect(ui.dialog).not.toBeNull();
    ui.dialog!.done("x" as never); expect(await p).toBe("x");
    expect(ui.dialog).toBeNull();
    const c = ui.confirm({ action: { id: "x", operation: "o" }, material: [] });
    ui.dialog!.done(false as never); expect(await c).toBe(false); expect(ui.dialog).toBeNull();
  });
  it("a dialog settled by a newer one leaves the newer one open", async () => {
    const ui = new ActionUi({} as never);
    const first = ui.prompt({ name: "a", title: "A", kind: "text" });
    void ui.choose({ name: "b", title: "B", options: [] });
    expect(await first).toBeNull(); expect(ui.dialog?.kind).toBe("choose");
  });
});
