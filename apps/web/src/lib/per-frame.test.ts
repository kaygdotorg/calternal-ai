import { afterEach, describe, expect, test, vi } from "vitest";
import { navHold } from "@calternal/ui";
import { perFrame } from "./per-frame";

describe("perFrame under a navigation hold", () => {
  afterEach(() => { navHold.release(); vi.useRealTimers(); });
  test("a held call is applied once after the hold, and no subscriber is left behind", () => {
    vi.useFakeTimers();
    const raf = vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((f) => { setTimeout(() => f(0), 16); return 1; });
    const seen: number[] = [];
    const feed = perFrame((n: number) => seen.push(n));
    navHold.begin();
    feed(1); feed(2); feed(3);
    expect(navHold.subscribers).toBe(1); // one, however many calls were held
    vi.advanceTimersByTime(300);
    expect(seen).toEqual([]);
    navHold.release();
    vi.advanceTimersByTime(300);
    expect(seen).toEqual([3]); // the latest, once
    expect(navHold.subscribers).toBe(0);
    raf.mockRestore();
  });
  test("many feeds (a store per space) that were each held leave no subscribers after the release", () => {
    vi.useFakeTimers();
    const raf = vi.spyOn(globalThis, "requestAnimationFrame").mockImplementation((f) => { setTimeout(() => f(0), 16); return 1; });
    navHold.begin();
    for (let i = 0; i < 20; i++) perFrame(() => {})(i); // each space switch makes a new feed
    expect(navHold.subscribers).toBe(20);
    navHold.release();
    vi.advanceTimersByTime(300);
    expect(navHold.subscribers).toBe(0);
    // and a feed that is never held never subscribes
    const quiet = perFrame(() => {}); quiet(1);
    expect(navHold.subscribers).toBe(0);
    raf.mockRestore();
  });
});
