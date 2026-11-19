import { beforeEach, describe, expect, test, vi } from "vitest";
import { flushSync } from "svelte";
import { threadsChrome } from "../../apps/threads/chrome";
import { view } from "../../apps/threads/view.svelte";
import { navigate, segments } from "../../lib/route.svelte";
import { answerIsNative, chromeCompatible, nativeChrome, PAGE_CONTRACT, registerChrome, routeEvent, startNativeChrome } from "./native-chrome.svelte";
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
    w.calternalNative = { chrome, log: vi.fn(), chromeVersions: [1] } as never;
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

describe("native chrome: the appearance and the schema form", () => {
  test("the appearance is the schema's: a wallpaper name, the three switches, the scale (a library picture is read once as a data URL)", async () => {
    const { appearanceOf } = await import("./appearance");
    const a = appearanceOf(() => {});
    expect(["string", "object"]).toContain(typeof a.wallpaper); // a name, or null for none
    expect(typeof a.blur).toBe("boolean");
    expect(a.uiSize).toBeGreaterThan(0);
    expect(a.material).toBe("wallpaper");
    expect(a.frost).toBeCloseTo(0.2);
  });
  test("the encoder keeps icons as names, badge values as strings, and windows a long section with count and offset", async () => {
    const { toSchema } = await import("./encode");
    const d = { version: 1 as const, revision: 3, scheme: "dark" as const, header: { title: "T" }, sidebar: { open: false, title: "C", selectors: [{ id: "f", selected: "a", options: [{ id: "a", title: "A", badge: { kind: "count" as const, value: 5 } }] }], sections: [{ id: "s", title: null, rows: [{ id: "r", title: "R", icon: "claude", trailing: [{ kind: "icon" as const, icon: "question", tone: "info" as const }, { kind: "count" as const, value: 2 }] }], count: 500, offset: 40 }], apps: [{ id: "threads", title: "Threads", icon: "threads", selected: true }], account: { id: "account", initial: "K", title: "Kay" }, search: { placeholder: "Search" } } };
    const out = toSchema(d) as { sidebar: { sections: { title?: string; count: number; offset: number; rows: { icon: string; trailing: { kind: string; value: string }[] }[] }[]; selectors: { options: { badge: string }[] }[] } };
    expect(out.sidebar.sections[0]!.title).toBeUndefined();
    expect(out.sidebar.sections[0]!.rows[0]!.icon).toBe("claude");
    expect(out.sidebar.sections[0]!.rows[0]!.trailing).toEqual([{ kind: "icon", value: "question", tone: "info" }, { kind: "count", value: "2" }]);
    expect([out.sidebar.sections[0]!.count, out.sidebar.sections[0]!.offset]).toEqual([500, 40]);
    expect(out.sidebar.selectors[0]!.options[0]!.badge).toBe("5");
  });
  test("a long section is sent as the window the shell asked for plus a margin", async () => {
    const { windowed } = await import("./native-chrome.svelte");
    const all = Array.from({ length: 300 }, (_, i) => i);
    nativeChrome.windows = { s: [100, 130] };
    const w = windowed("s", all);
    expect(w.count).toBe(300); expect(w.offset).toBe(80); expect(w.rows[0]).toBe(80); expect(w.rows.length).toBe(70);
    expect(windowed("short", all.slice(0, 10)).count).toBeUndefined();
    nativeChrome.windows = {};
  });
  test("rows are built only for the window (a thousand threads cost a window of rows)", async () => {
    const { windowedOf } = await import("./native-chrome.svelte");
    let built = 0;
    const all = Array.from({ length: 953 }, (_, i) => i);
    const w = windowedOf("inbox", all, (i) => { built++; return { id: String(i) }; });
    expect(w.count).toBe(953); expect(w.offset).toBe(0); expect(built).toBe(w.rows.length); expect(built).toBeLessThan(100);
    nativeChrome.windows = { inbox: [761, 909] }; // the shell asked for 148 rows at 761
    const big = windowedOf("inbox", all, (i) => ({ id: String(i) }));
    expect(big.rows.length).toBeLessThanOrEqual(60); expect(big.offset!).toBeGreaterThan(740); expect(big.offset! + big.rows.length).toBeGreaterThan(830);
    nativeChrome.windows = {};
  });
});

describe("native chrome: the compatibility rule", () => {
  const call = () => Promise.resolve({ native: true });
  test("no call, no chromeVersions (an older shell), an empty list or no common version: the page does not call and draws its own", () => {
    expect(chromeCompatible(undefined)).toBe(false);
    expect(chromeCompatible({})).toBe(false);
    expect(chromeCompatible({ chrome: call })).toBe(false);
    expect(chromeCompatible({ chrome: call, chromeVersions: [] })).toBe(false);
    expect(chromeCompatible({ chrome: call, chromeVersions: [2, 3] })).toBe(false);
  });
  test("a shell whose chromeVersions include the page's version is published to", () => {
    expect(chromeCompatible({ chrome: call, chromeVersions: [PAGE_CONTRACT] })).toBe(true);
    expect(chromeCompatible({ chrome: call, chromeVersions: [1, 2] })).toBe(true);
  });
  test("an answer is native only when it says native true; a refusal with a reason is off", () => {
    expect(answerIsNative({ native: true })).toBe(true);
    expect(answerIsNative({ native: false })).toBe(false);
    expect(answerIsNative({ native: false, reason: "unsupported-version", supported: [2] } as never)).toBe(false);
    expect(answerIsNative({} as never)).toBe(false);
    expect(answerIsNative(null)).toBe(false);
  });
  test("a refusal leaves the page's own chrome, and the log says why", async () => {
    const chrome = vi.fn().mockResolvedValue({ native: false, reason: "unsupported-version", supported: [2] });
    const log = vi.fn();
    w.calternalNative = { chrome, log, chromeVersions: [1] } as never;
    registerChrome("threads", threadsChrome);
    const stop = startNativeChrome(() => "threads");
    flushSync();
    await new Promise((r) => requestAnimationFrame(() => r(null)));
    await Promise.resolve(); await Promise.resolve();
    expect(nativeChrome.native).toBe(false);
    expect(log.mock.calls.some((c) => String(c[0]).includes("refused: unsupported-version"))).toBe(true);
    stop(); delete w.calternalNative;
  });
  test("a shell with no chromeVersions, or none in common, is never published to", () => {
    for (const bridge of [{ chrome: vi.fn() }, { chrome: vi.fn(), chromeVersions: [9] }]) {
      w.calternalNative = bridge as never;
      const stop = startNativeChrome(() => "threads");
      expect(bridge.chrome).not.toHaveBeenCalled();
      stop(); delete w.calternalNative;
    }
  });
});
