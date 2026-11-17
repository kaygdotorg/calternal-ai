// Toasts with a working Undo are never replaced; the next toast queues behind them (existing app #694), ported with the stack as the only presenter.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let notify: typeof import("../src/toast/notify.svelte").notify;
let views: typeof import("../src/toast/notify.svelte").views;
let stack: typeof import("../src/toast/stack.svelte");
const plain = (op: string) => notify({ op, severity: "result", message: op, awaited: true });
const bulk = (op: string, undo = vi.fn(async () => true)) => ({ id: notify({ op, severity: "result", message: "3 notes", undo, bulk: true }), undo });

describe("toast queue with Undo", () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    ({ notify, views } = await import("../src/toast/notify.svelte"));
    stack = await import("../src/toast/stack.svelte");
  });
  afterEach(() => vi.useRealTimers());

  it("a plain toast takes the place of the one before it", () => {
    plain("a");
    plain("b");
    expect(stack.ids.map((i) => views.get(i)?.text)).toEqual(["b"]);
  });
  it("repeats of the same event join one toast and count up", () => {
    const a = notify({ op: "x", context: "n1", severity: "result", message: "Deleted", awaited: true });
    const b = notify({ op: "x", context: "n1", severity: "result", message: "Deleted", awaited: true });
    expect(b).toBe(a);
    expect(views.get(a)?.count).toBe(2);
  });
  it("a toast with a working bulk Undo is not replaced: the next one waits behind it", async () => {
    const { id } = bulk("del");
    plain("later");
    expect(stack.ids).toEqual([id]);
    expect(views.get(id)?.action?.label).toBe("Undo");
    views.get(id)!.action!.run();
    await vi.advanceTimersByTimeAsync(10);
    expect(views.get(id)?.text).toBe("Undone");
  });
  it("Undo says exactly what it did when it cannot undo", async () => {
    const { id } = bulk("del", vi.fn(async () => false));
    views.get(id)!.action!.run();
    await vi.advanceTimersByTimeAsync(10);
    expect(views.get(id)?.text).toBe("Couldn't undo that");
    expect(views.get(id)?.phase).toBe("failed");
  });
  it("a wait turns into its result in place", async () => {
    const { track } = await import("../src/toast/notify.svelte");
    let done!: (v: string) => void;
    const id = track(new Promise<string>((r) => (done = r)), { op: "sync", loading: "Syncing", success: (v) => `Synced ${v}`, error: () => "Failed" });
    expect(views.get(id)?.phase).toBe("loading");
    done("3 files");
    await vi.advanceTimersByTimeAsync(0);
    expect(views.get(id)?.text).toBe("Synced 3 files");
  });
});
