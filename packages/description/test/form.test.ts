import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { MockHost } from "../testing/mock-host";
import { ACTIONS_KEY, ActionUi, HostSession } from "../src";
import type { FormNode as F } from "../src";
import FormNode from "../src/render/FormNode.svelte";

const node: F = { type: "form", id: "add", title: "Add a link", fields: [
  { id: "title", label: "Title", input: "text", required: true, maxLength: 20 }, { id: "url", label: "Address", input: "url", required: true } ],
  submit: { id: "add", title: "Add", operation: "reading.add", inputs: { title: { form: "add", field: "title" }, url: { form: "add", field: "url" } } } };
const flush = async () => { for (let i = 0; i < 30; i++) await vi.advanceTimersByTimeAsync(0); };
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

async function draw(outcome: "ok" | "error") {
  const host = new MockHost({ handle: (r, b: any) => r === "activation.resolve" ? { activation_id: b.activation_id, resolution_ref: "r" } : outcome === "ok" ? { activation_id: b.activation_id, state: "ok" } : { activation_id: b.activation_id, state: "error", code: "bad", message: "The host said no." } });
  const session = new HostSession({ transport: host }); await session.open("v");
  (session as any).viewContexts; // contexts: the mock issues none at open, so give the view one for the submit
  Object.assign(session.viewContexts, { add: "ctx-add" });
  const view = render(FormNode, { props: { node }, context: new Map([[ACTIONS_KEY, { ui: new ActionUi(session), session, viewContext: (id: string) => session.viewContexts[id] }]]) });
  const btn = () => view.container.querySelector<HTMLButtonElement>("[data-action=add]")!;
  const type = async (label: string, v: string) => { await fireEvent.input(view.container.querySelector<HTMLInputElement>(`label:has(.l) input[placeholder], input`)!, {}); const el = [...view.container.querySelectorAll("label")].find((l) => l.textContent?.includes(label))!.querySelector("input")!; await fireEvent.input(el, { target: { value: v } }); };
  return { host, view, btn, type };
}

describe("FormNode", () => {
  it("the submit is aria-disabled until required fields are valid, and checks the url shape and length", async () => {
    const { btn, type } = await draw("ok");
    expect(btn().getAttribute("aria-disabled")).toBe("true");
    await type("Title", "Read me"); await type("Address", "not a url");
    expect(btn().getAttribute("aria-disabled")).toBe("true");
    await type("Address", "https://example.org/x");
    expect(btn().getAttribute("aria-disabled")).toBeNull();
    await type("Title", "x".repeat(21));
    expect(btn().getAttribute("aria-disabled")).toBe("true");
  });

  it("values leave only in the submit's interaction; after ok the fields clear", async () => {
    const { host, view, btn, type } = await draw("ok");
    await type("Title", "Read me"); await type("Address", "https://example.org/x");
    await fireEvent.click(btn()); await flush();
    const resolve = host.log.find((l) => l.route === "activation.resolve")!.body as any;
    expect(resolve.interaction).toEqual({ form: { title: "Read me", url: "https://example.org/x" } });
    expect(resolve.activation_context).toBe("ctx-add");
    expect([...view.container.querySelectorAll("input")].map((i) => i.value)).toEqual(["", ""]);
  });

  it("after an error the fields keep what was typed and the host's message shows in an alert above the submit", async () => {
    const { view, btn, type } = await draw("error");
    await type("Title", "Read me"); await type("Address", "https://example.org/x");
    await fireEvent.click(btn()); await flush();
    expect(view.container.querySelector("[data-form-error]")?.textContent).toBe("The host said no.");
    expect([...view.container.querySelectorAll("input")].map((i) => i.value)).toEqual(["Read me", "https://example.org/x"]);
  });
});
