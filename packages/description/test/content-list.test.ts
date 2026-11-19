import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import { navHold } from "@calternal/ui";
import { MockHost } from "../testing/mock-host";
import { ACTIONS_KEY, ActionUi, ContentList, HostSession, Selection, WindowStore } from "../src";
import type { Bind } from "../src";

const base: Bind = {
  id: "items", source: { type: "list", kind: "reading-item" }, window: { size: 60, count: "exact" },
  row: { title: { field: "title" }, icon: { kindIcon: true }, marks: [{ kind: "icon", icon: "check", tone: "good", when: { field: "read", op: "eq", value: { literal: true } } }, { kind: "text", field: "due" }, { kind: "count", field: "n" }, { kind: "dot", field: "unread" }] },
  states: { empty: { title: "Nothing to read", detail: "Add a link below." }, error: { title: "No luck" } },
};
const flush = async () => { for (let i = 0; i < 30; i++) await vi.advanceTimersByTimeAsync(0); };
async function draw(host: MockHost, bind: Bind = base) {
  const session = new HostSession({ transport: host }); await session.open("v");
  const store = new WindowStore({ bind, source: session });
  const view = render(ContentList, { props: { store, selection: new Selection(bind.id, session), kindIcon: "notes", label: "Items" }, context: new Map([[ACTIONS_KEY, { ui: new ActionUi(session), session, viewContext: (id: string) => session.viewContexts[id] }]]) });
  return { store, view, session };
}
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); navHold.release(); });

describe("ContentList", () => {
  it("draws skeleton rows of the real row first, then exactly the rows the host sent", async () => {
    const host = new MockHost({ rows: 3, fields: (i) => ({ title: `Item ${i}`, read: i === 1, due: i === 2 ? "Fri" : "", n: i === 0 ? 4 : 0, unread: false }) });
    const { view } = await draw(host);
    expect(view.container.querySelectorAll("[data-skeleton]").length).toBeGreaterThan(0);
    await flush();
    const rows = [...view.container.querySelectorAll<HTMLElement>("[data-i]")];
    expect(rows.map((r) => r.querySelector(".t")?.textContent)).toEqual(["Item 0", "Item 1", "Item 2"]);
    expect(view.container.querySelectorAll("[data-skeleton]").length).toBe(0);
    // the kind's own icon leads every row; marks only where they hold
    expect(rows.every((r) => r.querySelector("svg[data-icon=notes]"))).toBe(true);
    expect(rows[0]!.querySelector("[data-mark=count]")?.textContent).toBe("4");
    expect(rows[1]!.querySelector("[data-mark=icon] svg[data-icon=check]")).toBeTruthy();
    expect(rows[2]!.querySelector("[data-mark=text]")?.textContent).toBe("Fri");
    expect(rows[0]!.querySelector("[data-mark=icon], [data-mark=text], [data-mark=dot]")).toBeNull();
    for (const r of rows) expect(r.querySelectorAll("[data-mark]").length).toBeLessThanOrEqual(3);
  });

  it("draws a template with no icon without an icon column", async () => {
    const { view } = await draw(new MockHost({ rows: 2 }), { ...base, row: { title: { field: "title" } } });
    await flush();
    expect(view.container.querySelector("[data-i] svg")).toBeNull();
    expect(view.container.querySelectorAll("[data-i]").length).toBe(2);
  });

  it("keeps the place of an icon the design set lacks", async () => {
    const { view } = await draw(new MockHost({ rows: 1 }), { ...base, row: { title: { field: "title" }, icon: "book" } });
    await flush();
    expect(view.container.querySelector("[data-icon-missing=book]")).toBeTruthy();
  });

  it("shows the bind's own empty message, and only when the whole result is empty", async () => {
    const { view } = await draw(new MockHost({ rows: 0 }));
    await flush();
    expect(view.container.querySelector("[data-state-message]")?.textContent).toContain("Nothing to read");
    expect(view.container.textContent).toContain("Add a link below.");
    expect(view.container.querySelector("[data-i]")).toBeNull();
  });

  it("shows the bind's error state as an alert when the host answers error", async () => {
    const host = new MockHost({ rows: 5, intercept: (req) => ({ audience: "own_space_member", bind: req.bind, state: "error", result_version: null, offset: 0, rows: [], window_ref: null }) as never });
    const { view } = await draw(host);
    await flush();
    const msg = view.container.querySelector("[data-state-message]")!;
    expect(msg.getAttribute("role")).toBe("alert"); expect(msg.textContent).toContain("No luck");
  });

  it("a typed plan refusal with no recovery shows the error state", async () => {
    const host = new MockHost({ rows: 5, intercept: () => ({ status: 422, code: "unsupported_query_plan", reason: "unsupported_plan" }) });
    const { view, store } = await draw(host);
    await flush();
    expect(store.state).toBe("error"); expect(store.planError).toBe("unsupported_plan");
    expect(view.container.querySelector("[data-state-message]")).toBeTruthy();
  });

  it("refine_query keeps the provisional rows and shows the refine message with no total and no sentinel", async () => {
    const host = new MockHost({ rows: 2, intercept: (req) => ({ audience: "own_space_member", bind: req.bind, state: "refine_query", result_version: null, offset: 0, rows: [{ id: "a", fields: { title: "A" } }, { id: "b", fields: { title: "B" } }], window_ref: null }) as never });
    const { view, store } = await draw(host);
    await flush();
    expect(store.state).toBe("refine_query"); expect(store.length).toBe(2);
    expect(view.container.querySelectorAll("[data-i]").length).toBe(2);
    expect(view.container.textContent).toContain("Refine your search.");
  });
});

describe("row image and secondary line", () => {
  const notes: Bind = { ...base, row: { title: { field: "title" }, secondary: [{ field: "snippet", format: "text" }], icon: { image: { field: "thumbnail", fallback: "snippet" } } } };
  it("draws the second line and, with no readable image, a miniature from the snippet, or the empty miniature: never a blank", async () => {
    const host = new MockHost({ rows: 2, fields: (i) => ({ title: `N${i}`, snippet: i === 0 ? "The quick brown fox" : "", thumbnail: `thumb-ref-${i}-aaaaaaaaaaaaaaaaaaaa` }) });
    const { view } = await draw(host, notes);
    await flush();
    const rows = [...view.container.querySelectorAll<HTMLElement>("[data-i]")];
    expect(rows[0]!.querySelector(".s")?.textContent).toBe("The quick brown fox");
    expect(rows[0]!.querySelector("[data-miniature=text]")?.textContent).toContain("The quick brown fox");
    expect(rows[1]!.querySelector("[data-miniature=blank]")).toBeTruthy();
    expect(rows.every((r) => r.querySelector("[data-row-image]"))).toBe(true);
  });
});
