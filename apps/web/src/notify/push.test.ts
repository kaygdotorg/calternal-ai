// Web push in the page (src/notify) and its worker (public/push-sw.js): where it is unavailable and what the person is told, the words a notification carries, and that the worker is push-only.
import { describe, expect, it } from "vitest";
import { fixturePush, supportOf } from "./push";
import { notificationLine } from "./payload";
import sw from "../../public/push-sw.js?raw";

const env = (o: Partial<Parameters<typeof supportOf>[0]> = {}) => ({ ua: "Mozilla/5.0 (X11; Linux x86_64) Chrome/130", standalone: false, secure: true, hasPush: true, hasNotification: true, ...o });
const IPHONE = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 Mobile/15E148 Safari/604.1";

describe("where push can work", () => {
  it("is available in a browser that has push on a secure address", () => expect(supportOf(env())).toBeNull());
  it("on an iPhone tab says to install the app first", () => { const u = supportOf(env({ ua: IPHONE })); expect(u?.reason).toBe("ios-not-installed"); expect(u?.text).toMatch(/Add to Home Screen/); });
  it("on an iPhone in the installed app is available", () => expect(supportOf(env({ ua: IPHONE, standalone: true }))).toBeNull());
  it("needs a secure address", () => expect(supportOf(env({ secure: false }))?.reason).toBe("insecure"));
  it("says so when the browser has no push", () => expect(supportOf(env({ hasPush: false }))?.reason).toBe("no-push"));
});

describe("what a notification says", () => {
  it("is 'needs you', 'finished' or a test, nothing else", () => {
    expect(notificationLine("needs_you")).toBe("needs you");
    expect(notificationLine("finished")).toBe("finished");
    expect(notificationLine("test")).toBe("Test notification");
    expect(notificationLine("anything else")).toBe("");
  });
  it("is the same in the worker", () => {
    for (const [status, line] of [["needs_you", "needs you"], ["finished", "finished"], ["test", "Test notification"]]) expect(sw).toContain(`status === "${status}" ? "${line}"`);
  });
  it("is made by a worker that only handles push and a tap: no fetch handler, no cache, no message text", () => {
    expect(sw).not.toMatch(/addEventListener\(\s*["']fetch["']/);
    expect(sw).not.toMatch(/\bcaches\b/);
    expect(sw).toMatch(/addEventListener\("push"/);
    expect(sw).toMatch(/addEventListener\("notificationclick"/);
    expect(sw).not.toMatch(/data\.(body|text|message)\b/);
  });
  // Runs the worker against a fake `self`: what a push shows and where a tap goes.
  function worker() {
    const on: Record<string, (e: unknown) => void> = {};
    const shown: { title: string; opts: { body: string; data: { thread: string; space: string } } }[] = [];
    const opened: string[] = [], navigated: string[] = [];
    let windows: { focus(): Promise<void>; navigate(p: string): Promise<void> }[] = [];
    const self = { addEventListener: (k: string, f: (e: unknown) => void) => { on[k] = f; }, skipWaiting() {}, clients: { claim() {}, matchAll: async () => windows, openWindow: async (p: string) => { opened.push(p); } }, registration: { showNotification: async (title: string, opts: never) => { shown.push({ title, opts }); } } };
    new Function("self", sw)(self);
    const run = async (k: string, e: object) => { let p: Promise<unknown> = Promise.resolve(); await on[k]({ ...e, waitUntil: (x: Promise<unknown>) => { p = x; } }); await p; };
    return { push: (d: object) => run("push", { data: { json: () => d } }), click: (data: object) => run("notificationclick", { notification: { close() {}, data } }), shown, opened, navigated, setWindows: (w: typeof windows) => { windows = w; } };
  }
  const T = "11111111-2222-3333-4444-555555555555", S = "66666666-7777-8888-9999-000000000000";
  it("shows only the title and 'needs you', keeps the ids out of the text", async () => {
    const w = worker(); await w.push({ title: "Fix the journal", status: "needs_you", thread_id: T, space_id: S });
    expect(w.shown[0]!.title).toBe("Fix the journal"); expect(w.shown[0]!.opts.body).toBe("needs you");
    expect(JSON.stringify([w.shown[0]!.title, w.shown[0]!.opts.body])).not.toContain(T);
    expect(w.shown[0]!.opts.data).toEqual({ thread: T, space: S });
  });
  it("a tap opens the thread in a new window, or navigates an open one", async () => {
    const w = worker(); await w.click({ thread: T }); expect(w.opened).toEqual([`/threads/${T}`]);
    const nav: string[] = []; w.setWindows([{ focus: async () => {}, navigate: async (p: string) => { nav.push(p); } }]); await w.click({ thread: T });
    expect(nav).toEqual([`/threads/${T}`]); expect(w.opened.length).toBe(1);
  });
  it("a test, or a notice with no usable id, opens the Inbox", async () => {
    const w = worker(); await w.push({ title: "Calternal", status: "test" }); expect(w.shown[0]!.opts.data).toEqual({ thread: "", space: "" });
    await w.click({ thread: "" }); await w.push({ title: "x", status: "finished", thread_id: "../evil" }); await w.click(w.shown[1]!.opts.data);
    expect(w.opened).toEqual(["/threads", "/threads"]);
  });
});

describe("the fixtures' account", () => {
  it("lists another device, adds this one, removes one, and limits tests to one a minute", async () => {
    const c = fixturePush();
    expect((await c.devices()).devices.map((d) => d.current)).toEqual([false]);
    expect((await c.subscribe({ endpoint: "x", p256dh: "", auth: "" })).some((d) => d.current)).toBe(true);
    await c.test();
    await expect(c.test()).rejects.toThrow(/minute/);
    expect(await c.remove("d-laptop")).toBe(true);
    expect(await c.remove("d-laptop")).toBe(false);
  });
});
