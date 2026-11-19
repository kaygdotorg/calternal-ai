import { beforeEach, describe, expect, test, vi } from "vitest";
import { flushSync } from "svelte";
import { threadsChrome } from "../../apps/threads/chrome";
import { view } from "../../apps/threads/view.svelte";
import { navigate, segments } from "../../lib/route.svelte";
import { nativeChrome, registerChrome, routeEvent, startNativeChrome } from "./native-chrome.svelte";
import { store } from "../../apps/threads/store.svelte";
import { notesChrome } from "../../apps/notes/chrome";
import { settingsChrome } from "../../apps/settings/chrome";
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
    expect(["claude", "codex"]).toContain(rows[0]!.icon); // the provider mark by its CalternalUI icon name
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
    const first = chrome.mock.calls[0]![0] as { revision: number; version: number; sidebar: { apps: unknown[]; account: { title: string } } };
    expect(first.version).toBe(1);
    expect(first.sidebar.apps.length).toBeGreaterThanOrEqual(2);
    expect(first.sidebar.account.title.length).toBeGreaterThan(0); // the account menu itself is only in the full contract (v2)
    expect(nativeChrome.native).toBe(true);
    nativeChrome.query = "x"; flushSync();
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    expect(chrome).toHaveBeenCalledTimes(2);
    expect((chrome.mock.calls[1]![0] as { revision: number }).revision).toBeGreaterThan(first.revision);
    stop(); delete w.calternalNative; nativeChrome.native = false; nativeChrome.query = "";
  });
});

describe("native chrome: the Notes and Settings providers", () => {
  test("Notes describes its shortcuts and sections, and opens a note, a collection or the Trash", () => {
    const b = notesChrome.describe("");
    expect(b.sections[0]!.rows.map((r) => r.id)).toEqual(["new", "trash"]);
    expect(b.search.placeholder).toBe("Find a note");
    notesChrome.handle({ type: "row", id: "trash", revision: 1, sentAt: 0 });
    expect(segments()).toEqual(["notes"]);
    notesChrome.handle({ type: "row", id: "c:col-1", revision: 2, sentAt: 0 });
    expect(segments()).toEqual(["collections", "col-1"]);
    notesChrome.handle({ type: "row", id: "n-1", revision: 3, sentAt: 0 });
    expect(segments()).toEqual(["notes", "n-1"]);
    navigate("/threads");
  });
  test("Settings lists the pages of its groups, narrowed by the search text, and opens one", () => {
    const all = settingsChrome.describe("").sections.flatMap((s) => s.rows);
    expect(all.length).toBeGreaterThan(3);
    const narrowed = settingsChrome.describe("appear").sections.flatMap((s) => s.rows);
    expect(narrowed.map((r) => r.id)).toContain("appearance");
    expect(narrowed.length).toBeLessThan(all.length);
    settingsChrome.handle({ type: "row", id: "appearance", revision: 1, sentAt: 0 });
    expect(segments()).toEqual(["settings", "appearance"]);
    expect(settingsChrome.describe("").header.title).toBe("Appearance");
    navigate("/threads");
  });
});

describe("native chrome: the appearance the shell paints with", () => {
  test("publishes the wallpaper, the switches, the scheme and the scale (and the solved veils when there are any)", async () => {
    const { appearanceOf } = await import("./appearance");
    document.documentElement.style.setProperty("--l-tint", "20 30 40"); document.documentElement.style.setProperty("--l-tint-a", "0.35");
    document.documentElement.style.setProperty("--l-dim", "10 10 10"); document.documentElement.style.setProperty("--l-dim-a", "0.5");
    document.documentElement.style.setProperty("--c-tint", "50 60 70"); document.documentElement.style.setProperty("--c-tint-a", "0.8");
    document.documentElement.style.setProperty("--c-dim", "255 255 255"); document.documentElement.style.setProperty("--c-dim-a", "0.6");
    const a = appearanceOf("dark");
    expect(a.scheme).toBe("dark");
    expect(typeof a.blur).toBe("boolean");
    expect(a.uiScale).toBeGreaterThan(0);
    expect(a.veils!.side.tint).toEqual([20, 30, 40, 0.35]);
    expect(a.veils!.card.dim).toEqual([255, 255, 255, 0.6]);
    for (const n of ["--l-tint", "--l-tint-a", "--l-dim", "--l-dim-a", "--c-tint", "--c-tint-a", "--c-dim", "--c-dim-a"]) document.documentElement.style.removeProperty(n);
    expect(appearanceOf("light").veils).toBeUndefined();
  });
});
