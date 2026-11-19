import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { MockHost } from "../testing/mock-host";
import { SyntheticSource } from "../testing/synthetic-source";
import { ACTIONS_KEY, APP_KEY, ActionUi, HostSession, hostComponents } from "../src";
import type { AppContext } from "../src";

const flush = async (ms = 0) => { for (let i = 0; i < 30; i++) await vi.advanceTimersByTimeAsync(ms); };
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

async function draw(type: "turn-transcript" | "composer" | "request-card", config: any, route: Record<string, string> = { thread: "thread-aaaa1111" }, over: Partial<{ appendEveryMs: number }> = {}) {
  const src = new SyntheticSource({ inner: new MockHost({ rows: 1 }), appendEveryMs: 2000, ...over });
  const session = new HostSession({ transport: src }); await session.open("v");
  const app = { route, storeFor: () => undefined, selectionFor: () => ({ drawn: null }), kindIconFor: () => undefined, setRoute() {}, components: {} } as unknown as AppContext;
  const C = hostComponents[type]!;
  const view = render(C as unknown as import("svelte").Component<any>, { props: { config, id: type }, context: new Map<any, any>([[ACTIONS_KEY, { ui: new ActionUi(session), session, viewContext: (id: string) => session.viewContexts[id] }], [APP_KEY, app]]) });
  return { src, session, view };
}

describe("turn-transcript", () => {
  it("reads the screenful at the end first, stamps the first draw, follows new committed events through the cursor", async () => {
    const { src, view } = await draw("turn-transcript", { thread: { route: "thread" }, follow: "end", window: 20 });
    await flush();
    expect(view.container.querySelectorAll("[data-event]").length).toBe(20);
    expect(document.documentElement.dataset.threadRendered).toBe("thread-aaaa1111");
    expect(view.container.textContent).toContain("Question 221"); // the last 20 of 240 events
    await vi.advanceTimersByTimeAsync(6500); await flush();
    expect(view.container.querySelectorAll("[data-event]").length).toBeGreaterThan(20);
    const pages = src.log.filter((l) => l.route === "transcript.page").map((l) => l.body as { after?: string });
    expect(pages[0]!.after).toBeUndefined(); expect(pages[1]!.after).toMatch(/^c\d+$/); // opaque cursor from the host, never made up
  });
  it("a refused cursor is dropped and the thread is read again from the end; no thread chosen reads nothing", async () => {
    const { src, view } = await draw("turn-transcript", { thread: { route: "thread" } }, { thread: "t-bbbbbbbb" });
    await flush();
    const orig = src.post.bind(src); let refused = false;
    src.post = (async (route: any, body: any, o: any) => { if (route === "transcript.page" && body.after && !refused) { refused = true; throw new (await import("../src")).HostError(410, "cursor_expired", "no"); } return orig(route, body, o); }) as never;
    await vi.advanceTimersByTimeAsync(4000); await flush();
    expect(refused).toBe(true); expect(view.container.querySelectorAll("[data-event]").length).toBeGreaterThan(0);
    const none = await draw("turn-transcript", { thread: { route: "thread" } }, {});
    await flush(); expect(none.src.log.length).toBe(0); expect(none.view.container.textContent).toContain("Choose a thread");
  });
});

describe("composer", () => {
  it("lists only the options the host returned; the draft never leaves until Send; Send is resolve (material) then one invoke_confirm with final_text; the draft clears after ok", async () => {
    const { src, session, view } = await draw("composer", { thread: { route: "thread" }, controls: ["provider", "model", "stop"], placeholder: "Message" });
    Object.assign(session.viewContexts, { composer: "ctx-composer" });
    await flush();
    const selects = [...view.container.querySelectorAll<HTMLSelectElement>("select[data-control]")];
    expect(selects.map((s) => s.dataset.control)).toEqual(["provider", "model"]); // stop is a button of the host's, not an option list
    expect([...selects[1]!.options].map((o) => o.textContent)).toEqual(["Sonnet 5.5", "Opus 5.5", "Haiku 5.5"]);
    const ta = view.container.querySelector("textarea")!;
    await fireEvent.input(ta, { target: { value: "hello there" } });
    expect(src.log.some((l) => l.route.startsWith("activation"))).toBe(false); // typing sends nothing
    await fireEvent.click(view.container.querySelector("[data-send]")!); await flush();
    const routes = src.log.filter((l) => l.route.startsWith("activation")).map((l) => l.route);
    expect(routes).toEqual(["activation.resolve", "activation.confirm"]);
    const resolve = src.log.find((l) => l.route === "activation.resolve")!.body as any, confirm = src.log.find((l) => l.route === "activation.confirm")!.body as any;
    expect(JSON.stringify(resolve)).not.toContain("hello there"); // the words are not part of the prepared material
    expect(confirm.final_text).toBe("hello there");
    expect(Object.keys(resolve.interaction)).toEqual(["options"]);
    expect(ta.value).toBe("");
  });
  it("keeps the draft and shows the host's message when the send fails", async () => {
    const { src, view } = await draw("composer", { thread: { route: "thread" } });
    const orig = src.post.bind(src);
    src.post = (async (route: any, body: any, o: any) => (route === "activation.confirm" ? { activation_id: "a", state: "error", code: "no", message: "The thread is archived." } : orig(route, body, o))) as never;
    const ta = view.container.querySelector("textarea")!;
    await fireEvent.input(ta, { target: { value: "keep me" } });
    await fireEvent.click(view.container.querySelector("[data-send]")!); await flush();
    expect(ta.value).toBe("keep me"); expect(view.container.querySelector("[data-composer-error]")?.textContent).toBe("The thread is archived.");
  });
});

describe("request-card", () => {
  it("shows the host's material and only its own buttons resolve it, with the card context and request ref", async () => {
    const { src, view } = await draw("request-card", { request: { route: "request" } }, { request: "req-1" });
    await flush();
    expect(view.container.querySelectorAll("[data-material] dt").length).toBe(2);
    await fireEvent.click([...view.container.querySelectorAll("button")].find((b) => b.textContent === "Approve for this thread")!); await flush();
    const r = src.log.find((l) => l.route === "request.resolve")!.body as any;
    expect(r).toEqual({ type: "request_resolve", card_context: "synthetic-card", request_ref: "req-1", decision: "approve", lifetime: "this_thread" });
  });
  it("deny carries no lifetime", async () => {
    const { src, view } = await draw("request-card", { request: { route: "request" } }, { request: "req-2" });
    await flush();
    await fireEvent.click([...view.container.querySelectorAll("button")].find((b) => b.textContent === "Deny")!); await flush();
    expect((src.log.find((l) => l.route === "request.resolve")!.body as any).lifetime).toBeUndefined();
  });
});
