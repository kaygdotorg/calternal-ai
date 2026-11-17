// The toast stack: replace/stack, per-toast holds, the visible cap, timers cleaned up.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

let s: typeof import("../src/toast/stack.svelte");
let gone: string[];

describe("toast stack", () => {
  beforeEach(async () => {
    vi.resetModules();
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    s = await import("../src/toast/stack.svelte");
    gone = [];
    s.onStackEnd((id) => gone.push(id));
  });
  afterEach(() => vi.useRealTimers());

  it("a toast leaves when its time is up", () => {
    s.stackPresenter.show("a", 1000);
    vi.advanceTimersByTime(999);
    expect(s.ids).toEqual(["a"]);
    vi.advanceTimersByTime(1);
    expect(s.ids).toEqual([]);
    expect(gone).toEqual(["a"]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("showing the same id again keeps one toast and restarts its time", () => {
    s.stackPresenter.show("a", 1000);
    vi.advanceTimersByTime(800);
    s.stackPresenter.show("a", 1000);
    vi.advanceTimersByTime(800);
    expect(s.ids).toEqual(["a"]);
    vi.advanceTimersByTime(200);
    expect(s.ids).toEqual([]);
  });

  it("holding stops the time; letting go carries on with what was left", () => {
    s.stackPresenter.show("a", 1000);
    vi.advanceTimersByTime(600);
    s.hold("a", "mouse", true);
    vi.advanceTimersByTime(5000);
    expect(s.ids).toEqual(["a"]);
    s.hold("a", "mouse", false);
    vi.advanceTimersByTime(399);
    expect(s.ids).toEqual(["a"]);
    vi.advanceTimersByTime(1);
    expect(s.ids).toEqual([]);
  });

  it("stays held until every reason lets go (mouse and focus)", () => {
    s.stackPresenter.show("a", 1000);
    s.hold("a", "mouse", true);
    s.hold("a", "focus", true);
    s.hold("a", "mouse", false);
    vi.advanceTimersByTime(5000);
    expect(s.ids).toEqual(["a"]);
    s.hold("a", "focus", false);
    vi.advanceTimersByTime(1000);
    expect(s.ids).toEqual([]);
  });

  it("a toast changed in place while held starts again unheld (Undo -> Undone)", () => {
    s.stackPresenter.show("a", 8000);
    s.hold("a", "mouse", true);
    s.stackPresenter.show("a", 1800);
    vi.advanceTimersByTime(1800);
    expect(s.ids).toEqual([]);
  });

  it("a hold belongs to its toast: the next one starts unheld", () => {
    s.stackPresenter.show("a", 1000);
    s.hold("a", "mouse", true);
    s.stackPresenter.dismiss("a");
    s.stackPresenter.show("b", 1000);
    vi.advanceTimersByTime(1000);
    expect(s.ids).toEqual([]);
    expect(gone).toEqual(["a", "b"]);
  });

  it("a wait (no end) stays until dismissed, with no timer", () => {
    s.stackPresenter.show("w", Infinity);
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(60_000);
    expect(s.ids).toEqual(["w"]);
    s.stackPresenter.dismiss("w");
    expect(s.ids).toEqual([]);
  });

  it("only the newest few are drawn; the older ones still keep time and leave", () => {
    for (const id of ["a", "b", "c", "d", "e"]) s.stackPresenter.show(id, 1000);
    expect(s.ids.length).toBe(5);
    expect(s.visible()).toEqual(["c", "d", "e"]);
    vi.advanceTimersByTime(1000);
    expect(s.ids).toEqual([]);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("dismissing clears its timer", () => {
    s.stackPresenter.show("a", 1000);
    s.stackPresenter.show("b", 1000);
    s.stackPresenter.dismiss("a");
    expect(vi.getTimerCount()).toBe(1);
    expect(s.ids).toEqual(["b"]);
  });
});
