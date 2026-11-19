import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockHost } from "../testing/mock-host";
import { HostSession, Invoker, Selection, fullSwipeAllowed, oneExchange } from "../src";
import type { ActionOutcome, Invoke } from "../src";
import type { InvokerHooks } from "../src";

const pin: Invoke = { id: "pin", title: "Pin", operation: "host.item.patch-metadata", inputs: { item: { row: "id" }, field: { literal: "pinned" }, value: { literal: true } } };
const add: Invoke = { id: "add", title: "Add", operation: "reading.add", inputs: { title: { form: "add", field: "title" } }, then: { route: { name: "item" } } };
const ok = (id: string, extra: Partial<ActionOutcome> = {}): ActionOutcome => ({ activation_id: id, state: "ok", ...extra });

function setup(handle: (route: string, body: any) => unknown, hooks: Partial<InvokerHooks> = {}) {
  const host = new MockHost({ handle: (r, b) => handle(r, b) });
  const session = new HostSession({ transport: host });
  const calls = () => host.log.filter((l) => l.route.startsWith("activation") || ["choice.options", "status", "undo"].includes(l.route));
  const h: InvokerHooks = { prompt: async () => "x", choose: async () => null, confirm: async () => true, ...hooks };
  return { host, session, calls, inv: new Invoker(session, h) , open: () => session.open("v") };
}
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

describe("catalogue gates", () => {
  it("one exchange and full swipe come from the catalogue, never the description", () => {
    expect(oneExchange("host.item.patch-metadata")).toBe(true);
    expect(oneExchange("host.item.trash")).toBe(true);
    expect(oneExchange("reading.add")).toBe(false);
    expect(fullSwipeAllowed("host.item.trash")).toBe(true);
    expect(fullSwipeAllowed("host.item.rename")).toBe(false);
    expect(fullSwipeAllowed("reading.add")).toBe(false);
  });
});

describe("Invoker", () => {
  it("an eligible built-in is one exchange: resolve with dispatch:true, outcome straight back", async () => {
    const s = setup((_r, b) => ok(b.activation_id, { undo: { ref: "u1", label: "Pinned", expires_at: "2026-11-19T10:00:00Z" } }));
    await s.open();
    const out = await s.inv.run(pin, { context: "ctx-1" }) as ActionOutcome;
    expect(out.state).toBe("ok");
    const c = s.calls(); expect(c.length).toBe(1);
    expect(c[0]!.route).toBe("activation.resolve"); expect((c[0]!.body as any).dispatch).toBe(true);
    expect((c[0]!.body as any).activation_context).toBe("ctx-1");
  });

  it("a plugin operation is resolve then dispatch, no dispatch flag, no sheet; the activation id is the same", async () => {
    let confirms = 0;
    const s = setup((r, b) => (r === "activation.resolve" ? { activation_id: b.activation_id, resolution_ref: "res-1" } : ok(b.activation_id)), { confirm: async () => { confirms++; return true; } });
    await s.open();
    await s.inv.run(add, { context: "vctx", interaction: { form: { title: "T" } } });
    const c = s.calls(); expect(c.map((x) => x.route)).toEqual(["activation.resolve", "activation.dispatch"]);
    expect((c[0]!.body as any).dispatch).toBeUndefined(); expect(confirms).toBe(0);
    expect((c[1]!.body as any).activation_id).toBe((c[0]!.body as any).activation_id);
  });

  it("sends only device-held values: no row, route, selection, target or viewer ever leaves", async () => {
    const s = setup((r, b) => (r === "activation.resolve" ? { activation_id: b.activation_id, resolution_ref: "res" } : ok(b.activation_id)));
    await s.open();
    await s.inv.run(add, { context: "c", interaction: { form: { title: "T" } } });
    const body = s.calls()[0]!.body as Record<string, unknown>;
    expect(Object.keys(body).sort()).toEqual(["action_id", "activation_context", "activation_id", "interaction", "type"]);
    expect(Object.keys(body.interaction as object)).toEqual(["form"]);
  });

  it("a challenge shows the host's material in the protected step; confirm returns the challenge; dispatch carries the proof (three exchanges)", async () => {
    let shown: unknown;
    const s = setup((r, b) => r === "activation.resolve" ? { activation_id: b.activation_id, resolution_ref: "res", confirmation_challenge: "chal", material: [{ name: "to", label: "Recipient", value: "ann" }] } : r === "activation.confirm" ? { confirmation_proof: "proof" } : ok(b.activation_id), { confirm: async (x) => { shown = x.material; return true; } });
    await s.open();
    const out = await s.inv.run(add, { context: "c" }) as ActionOutcome;
    expect(out.state).toBe("ok"); expect(shown).toEqual([{ name: "to", label: "Recipient", value: "ann" }]);
    const c = s.calls(); expect(c.map((x) => x.route)).toEqual(["activation.resolve", "activation.confirm", "activation.dispatch"]);
    expect((c[1]!.body as any).confirmation_challenge).toBe("chal"); expect((c[2]!.body as any).confirmation_proof).toBe("proof");
  });

  it("declining the confirmation sends nothing further", async () => {
    const s = setup((r, b) => ({ activation_id: b.activation_id, resolution_ref: "res", confirmation_challenge: "chal", material: [] }), { confirm: async () => false });
    await s.open();
    expect(await s.inv.run(add, { context: "c" })).toEqual({ state: "cancelled" });
    expect(s.calls().length).toBe(1);
  });

  it("waits and sends nothing when no host-issued context matches the drawn row; a held selection is flushed first", async () => {
    const s = setup((_r, b) => ok(b.activation_id));
    await s.open();
    expect(await s.inv.run(pin, { context: undefined })).toEqual({ state: "waiting" });
    expect(s.calls().length).toBe(0);
    const sel = new Selection("notes", { lease: s.session.lease, selection: (bind, row) => Promise.resolve({ type: "selection_context", lease_ref: s.session.lease!.lease_ref, cache_partition: s.session.lease!.cache_partition, bind, row, activation_contexts: { pin: `ctx-${row}` } }) });
    sel.paint("r-9"); // drawn, not yet settled
    const out = await s.inv.run(pin, { context: undefined, flush: () => sel.flush(), recontext: () => sel.contextFor("pin") }) as ActionOutcome;
    expect(out.state).toBe("ok"); expect((s.calls()[0]!.body as any).activation_context).toBe("ctx-r-9");
  });

  it("prompts and choices go in interaction under the input's name; the choice is a host-issued ref fetched first", async () => {
    const move: Invoke = { id: "move", title: "Move", operation: "host.collection.move", inputs: { item: { row: "id" }, collection: { prompt: { title: "Move to", kind: "choice", options: { bind: "collections" } } } } };
    const ren: Invoke = { id: "rename", title: "Rename", operation: "host.item.rename", inputs: { item: { row: "id" }, title: { prompt: { title: "Rename", initial: { literal: "Old" } } } } };
    let initial: string | undefined;
    const s = setup((r, b) => r === "choice.options" ? { bind: "collections", options: [{ ref: "ch-1", label: "Work" }] } : ok(b.activation_id), { choose: async (x) => x.options[0]!.ref, prompt: async (x) => { initial = x.initial; return "New"; } });
    await s.open();
    await s.inv.run(move, { context: "c" });
    expect(s.calls().map((x) => x.route)).toEqual(["choice.options", "activation.resolve"]);
    expect((s.calls()[0]!.body as any)).toMatchObject({ type: "choice_options", activation_context: "c", slot: "collection" });
    expect((s.calls()[1]!.body as any).interaction).toEqual({ choices: { collection: "ch-1" } });
    await s.inv.run(ren, { context: "c2" });
    expect((s.calls().at(-1)!.body as any).interaction).toEqual({ prompt: { title: "New" } }); expect(initial).toBe("Old");
  });

  it("pending is polled through the host's status route until it settles", async () => {
    let n = 0;
    const s = setup((r, b) => r === "status" ? (++n < 2 ? { activation_id: b.activation_id ?? "a", state: "pending", status_ref: "st" } : ok("a")) : { activation_id: b.activation_id, state: "pending", status_ref: "st" });
    await s.open();
    const p = s.inv.run(pin, { context: "c" });
    await vi.advanceTimersByTimeAsync(5000);
    expect(((await p) as ActionOutcome).state).toBe("ok"); expect(s.calls().filter((x) => x.route === "status").length).toBe(2);
  });

  it("error, refused, stale and unknown outcomes reach the display; a dropped connection is unknown and never retried", async () => {
    const seen: string[] = [];
    for (const state of ["error", "refused", "stale"] as const) {
      const s = setup((_r, b) => ({ activation_id: b.activation_id, state, code: "x", message: `host says ${state}` }), { outcome: (_a, o) => seen.push(`${o.state}:${o.message}`) });
      await s.open(); await s.inv.run(pin, { context: "c" });
    }
    const s = setup(() => { throw new Error("offline"); }, { outcome: (_a, o) => seen.push(o.state) });
    await s.open(); const out = await s.inv.run(pin, { context: "c" }) as ActionOutcome;
    expect(out.state).toBe("unknown"); expect(s.calls().length).toBe(1);
    expect(seen).toEqual(["error:host says error", "refused:host says refused", "stale:host says stale", "unknown"]);
  });

  it("created opens through then.route; a second activation of the same action is ignored while one runs", async () => {
    const routed: string[] = [];
    let release!: () => void;
    const s = setup((r, b) => r === "activation.resolve" ? { activation_id: b.activation_id, resolution_ref: "res" } : new Promise((res) => { release = () => res(ok(b.activation_id, { created: "item-7" })); }), { route: (n, c) => routed.push(`${n}=${c}`) });
    await s.open();
    const first = s.inv.run(add, { context: "c" });
    await vi.advanceTimersByTimeAsync(0);
    expect(await s.inv.run(add, { context: "c" })).toEqual({ state: "waiting" });
    release(); await first;
    expect(routed).toEqual(["item=item-7"]);
  });
});
