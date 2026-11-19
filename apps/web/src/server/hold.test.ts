import { describe, expect, test } from "vitest";
import { ServerClient } from "./client";
import { link } from "./link.svelte";

// The app is painted from the device's record before the server confirms the session: nothing may be changed until it does, and reads (which only a live session is answered) go ahead.
function client() {
  const sent: string[] = [];
  const c = new ServerClient({ token: () => null, fetch: (async (url: string, init?: RequestInit) => { sent.push(`${init?.method ?? "GET"} ${url}${init?.body ? ` ${(JSON.parse(String(init.body)) as { operation?: string }).operation ?? ""}` : ""}`); return new Response(JSON.stringify({ ok: true, data: {} }), { status: 200 }); }) as typeof fetch });
  return { c, sent };
}

describe("no change is sent before the session is confirmed", () => {
  test("a write waits, a read goes, and the write is sent once the server confirms", async () => {
    const { c, sent } = client();
    let confirm!: (ok: boolean) => void;
    link.holdWrites(new Promise<boolean>((r) => (confirm = r)));
    const write = c.operate("notes.update", { id: "n" });
    await c.operate("notes.list", { space_id: "s" });
    await c.json("/api/v1/machines").catch(() => {});
    await Promise.resolve();
    expect(sent.some((s) => s.includes("notes.update"))).toBe(false);
    expect(sent.some((s) => s.includes("notes.list"))).toBe(true);
    confirm(true);
    await write;
    expect(sent.some((s) => s.includes("notes.update"))).toBe(true);
  });
  test("when the server says the session is gone the held change is refused and never sent", async () => {
    const { c, sent } = client();
    let confirm!: (ok: boolean) => void;
    link.holdWrites(new Promise<boolean>((r) => (confirm = r)));
    const write = c.operate("threads.send", { id: "t", message: "hi" });
    const code = c.json("/api/v1/machines/codes", { body: {} });
    confirm(false);
    await expect(write).rejects.toMatchObject({ code: "unauthorized" });
    await expect(code).rejects.toMatchObject({ code: "unauthorized" });
    expect(sent).toEqual([]);
  });
});

describe("a read the last word does not tell is not held", () => {
  test("onboarding.state goes while the session is unconfirmed (the warm start-up read), a mutation still waits", async () => {
    const { c, sent } = client();
    let confirm!: (ok: boolean) => void;
    link.holdWrites(new Promise<boolean>((r) => (confirm = r)));
    const write = c.operate("onboarding.finish", {});
    await c.operate("onboarding.state", {});
    await Promise.resolve();
    expect(sent.some((s) => s.includes("onboarding.state"))).toBe(true);
    expect(sent.some((s) => s.includes("onboarding.finish"))).toBe(false);
    confirm(true);
    await write;
  });
});
