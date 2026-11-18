import { describe, expect, it, vi } from "vitest";
import { FixtureSharing, expiryText, sentenceOf } from "./sharing";
import { qrMatrix, qrSvg } from "../../../lib/qr";

const item = { kind: "notes" as const, id: "n-9" };

describe("the Share sheet's sentence", () => {
  it("says only you when nothing is shared", () => expect(sentenceOf({ grants: [], space_access: null })).toBe("Only you can see this."));
  it("names people by what they can do, then links and the space", () => {
    const g = (name: string, permission: "view" | "edit", audience: { account_id: string }) => ({ grant_id: name, name, permission, audience });
    const link = { grant_id: "l", label: "Link 1", permission: "view" as const, expires_at: null };
    expect(sentenceOf({ space_access: "view", grants: [g("Maya", "edit", { account_id: "a" }), g("Theo", "view", { account_id: "b" }), g("Inés", "view", { account_id: "c" }), link] }))
      .toBe("Everyone in this space can view. Maya can edit. Theo and Inés can view. Anyone with a link can view.");
  });
});

describe("expiry words", () => {
  it("says Never, Expired or the day", () => {
    expect(expiryText(null)).toBe("Never");
    expect(expiryText(1000, 2000)).toBe("Expired");
    expect(expiryText(Date.UTC(2026, 10, 24), Date.UTC(2026, 10, 20))).toBe("Ends 24 Nov");
  });
});

describe("the fixture sharing backend", () => {
  it("grants, changes, links and revokes", async () => {
    const s = new FixtureSharing();
    await s.grant(item, { account_id: "acc-maya" }, "view");
    await s.grant(item, { account_id: "acc-maya" }, "edit");
    const link = await s.link_create(item, "Link 1");
    let l = await s.list(item);
    expect(l.grants.map((g) => ["label" in g ? g.label : g.name, g.permission])).toEqual([["Maya Okafor", "edit"], ["Link 1", "view"]]);
    expect((await s.audiences(item, "maya")).audiences).toEqual([]); // already granted
    expect((await s.link_copy(item, link.grant_id)).url).toMatch(/\/s\/fx\d+/);
    await s.link_update(item, link.grant_id, undefined, 5);
    expect((await s.list(item)).grants[1]).toMatchObject({ expires_at: 5 });
    await s.revoke(item, link.grant_id);
    l = await s.list(item);
    expect(l.grants).toHaveLength(1);
  });
  it("a collection lists its members and a member leads back", async () => {
    const s = new FixtureSharing();
    expect(await s.guest("/s/demo-set")).toMatchObject({ members: [{ href: "/s/demo-set/1" }, { href: "/s/demo-set/2" }] });
    expect(await s.guest("/s/demo-set/1")).toMatchObject({ back: "/s/demo-set" });
  });
  it("a secret that does not work is not_found", async () => { await expect(new FixtureSharing().guest("/s/nope")).rejects.toMatchObject({ code: "not_found" }); });
});

describe("the QR encoder", () => {
  it("draws a link and refuses text that is too long", () => {
    expect(qrMatrix("https://calternal.example/s/abcdef")?.length).toBeGreaterThanOrEqual(21);
    expect(qrSvg("x".repeat(300))).toBe("");
    expect(qrSvg("https://calternal.example/s/abcdef")).toContain("<svg");
  });
});

describe("the real client's guest read", () => {
  it("asks for JSON, and a 404 is not_found whatever the reason", async () => {
    const { RealSharing } = await import("./real");
    const seen: string[] = [];
    vi.stubGlobal("fetch", async (url: string, init: RequestInit) => { seen.push(`${url} ${(init.headers as Record<string, string>).Accept}`); return url.endsWith("/s/ok") ? new Response(JSON.stringify({ title: "T", markdown: "x" })) : new Response("", { status: 404 }); });
    const c = new RealSharing();
    expect(await c.guest("/s/ok")).toEqual({ title: "T", markdown: "x" });
    await expect(c.guest("/s/gone")).rejects.toMatchObject({ code: "not_found" });
    expect(seen[0]).toMatch(/\/s\/ok application\/json$/);
    vi.unstubAllGlobals();
  });
});

import { blocksOf, changedBlocks, rescueLine, textKey } from "./rescue";
describe("rescue", () => {
  it("keeps only the blocks the person changed", () => {
    const base = "# T\n\nOne.\n\nTwo.";
    expect(changedBlocks(base, "# T\n\nOne.\n\nTwo, changed.\n\nNew.")).toEqual(["Two, changed.", "New."]);
    expect(changedBlocks(base, base)).toEqual([]);
    expect(blocksOf("a\n \nb\n\n\nc")).toEqual(["a", "b", "c"]);
  });
  it("makes the same text the same request, and says where the edits went", () => {
    expect(textKey("abc")).toBe(textKey("abc"));
    expect(textKey("abc")).not.toBe(textKey("abd"));
    expect(rescueLine("Plan", "Your edits to Plan")).toBe("You can no longer open “Plan”. Your edits were kept as “Your edits to Plan” in your personal space.");
  });
});
