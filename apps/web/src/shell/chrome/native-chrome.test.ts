import { beforeEach, describe, expect, test, vi } from "vitest";
import { flushSync } from "svelte";
import { threadsChrome } from "../../apps/threads/chrome";
import { view } from "../../apps/threads/view.svelte";
import { navigate, segments } from "../../lib/route.svelte";
import { nativeChrome, registerChrome, routeEvent, startNativeChrome } from "./native-chrome.svelte";
import { store } from "../../apps/threads/store.svelte";
import type { ThreadSummary } from "../../apps/threads/model";
import { spaces } from "../spaces.svelte";

// The Threads provider describes the person's threads for the shell and answers its events with the same functions the drawn sidebar uses; the publisher sends a new description when a store it reads changes.
type W = { calternalNative?: { chrome?: ReturnType<typeof vi.fn>; log?: ReturnType<typeof vi.fn> } };
const w = window as unknown as W;

const mk = (id: string, title: string, over: Partial<ThreadSummary> = {}): ThreadSummary => ({ id, spaceId: spaces.current.id, title, parentId: null, state: "idle", workspace: "w", updatedAt: Date.now(), provider: "claude", ...over }) as ThreadSummary;

describe("native chrome: the Threads provider", () => {
  beforeEach(() => { view.chosen = "all"; nativeChrome.query = ""; store.threads = [mk("t-a", "Plan the storage layout"), mk("t-b", "Passkey flake", { state: "running", provider: "codex" }), mk("t-c", "Release checklist")]; });
  test("describes the header, the filter, the shortcuts and the person's threads as rows", () => {
    const b = threadsChrome.describe("");
    expect(b.selectors[0]!.options.map((o) => o.id)).toEqual(["inbox", "all"]);
    expect(b.sections[0]!.id).toBe("top");
    const rows = b.sections.flatMap((s) => s.rows).filter((r) => store.get(r.id));
    expect(rows.length).toBeGreaterThan(0);
    expect(rows[0]!.icon).toHaveProperty("svg");
    expect(rows[0]!.menu!.some((m) => m.id === "rename" && m.input)).toBe(true);
    expect(rows[0]!.trailingActions!.some((a) => a.id === "archive" && a.role === "destructive")).toBe(true);
  });
  test("the search text narrows the rows, and the Inbox choice swaps the sections", () => {
    const all = threadsChrome.describe("").sections.flatMap((s) => s.rows).filter((r) => store.get(r.id));
    const one = all[0]!;
    const narrowed = threadsChrome.describe(one.title.slice(0, 6)).sections.flatMap((s) => s.rows).filter((r) => store.get(r.id));
    expect(narrowed.map((r) => r.id)).toContain(one.id);
    expect(narrowed.length).toBeLessThanOrEqual(all.length);
    view.chosen = "inbox";
    expect(threadsChrome.describe("").sections.map((s) => s.id)).toContain("inbox");
  });
  test("a row event opens the thread, the selector event changes the tab", () => {
    const id = store.threads[0]!.id;
    threadsChrome.handle({ type: "row", id, revision: 1, sentAt: Date.now() });
    expect(segments()).toEqual(["threads", id]);
    threadsChrome.handle({ type: "selector", id: "filter", option: "inbox", revision: 2, sentAt: Date.now() });
    expect(view.chosen).toBe("inbox");
  });
  test("the shell's own events: an app opens its address, the sidebar and the search are remembered", () => {
    registerChrome("threads", threadsChrome);
    routeEvent("threads", { type: "app", id: "notes", revision: 1, sentAt: 0 });
    expect(segments()[0]).toBe("notes");
    routeEvent("threads", { type: "sidebar", open: true, revision: 2, sentAt: 0 });
    expect(nativeChrome.open).toBe(true);
    routeEvent("threads", { type: "search", text: "plan", revision: 3, sentAt: 0 });
    expect(nativeChrome.query).toBe("plan");
    nativeChrome.open = false; nativeChrome.query = ""; navigate("/threads");
  });
});

describe("native chrome: the publisher", () => {
  test("does nothing in a browser (no bridge)", () => {
    delete w.calternalNative;
    expect(startNativeChrome(() => "threads")).toBeTypeOf("function");
  });
  test("publishes a description with a rising revision when a store changes, and keeps the shell's answer", async () => {
    const chrome = vi.fn().mockResolvedValue({ native: true });
    w.calternalNative = { chrome, log: vi.fn() };
    registerChrome("threads", threadsChrome);
    view.chosen = "all";
    const stop = startNativeChrome(() => "threads");
    flushSync();
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    await Promise.resolve();
    expect(chrome).toHaveBeenCalledTimes(1);
    const first = chrome.mock.calls[0]![0] as { revision: number; version: number; sidebar: { apps: unknown[]; account: { menu: unknown[] } } };
    expect(first.version).toBe(1);
    expect(first.sidebar.apps.length).toBeGreaterThanOrEqual(2);
    expect(first.sidebar.account.menu.length).toBeGreaterThan(2);
    expect(nativeChrome.native).toBe(true);
    nativeChrome.query = "x"; flushSync();
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    expect(chrome).toHaveBeenCalledTimes(2);
    expect((chrome.mock.calls[1]![0] as { revision: number }).revision).toBeGreaterThan(first.revision);
    stop(); delete w.calternalNative; nativeChrome.native = false; nativeChrome.query = "";
  });
});
