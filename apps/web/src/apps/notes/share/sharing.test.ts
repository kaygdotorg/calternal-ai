import { describe, expect, it } from "vitest";
import { FixtureSharing, expiryText, sentenceOf } from "./sharing";
import { qrMatrix, qrSvg } from "../../../lib/qr";

const item = { kind: "notes" as const, id: "n-9" };

describe("the Share sheet's sentence", () => {
  it("says only you when nothing is shared", () => expect(sentenceOf({ grants: [], space_access: null })).toBe("Only you can see this."));
  it("names people by what they can do, then links and the space", () => {
    const g = (name: string, permission: "view" | "edit", audience?: { account_id: string }) => ({ grant_id: name, name, permission, audience, expires_at: null });
    expect(sentenceOf({ space_access: "view", grants: [g("Maya", "edit", { account_id: "a" }), g("Theo", "view", { account_id: "b" }), g("Inés", "view", { account_id: "c" }), g("Link 1", "view")] }))
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
    expect(l.grants.map((g) => [g.name, g.permission])).toEqual([["Maya Okafor", "edit"], ["Link 1", "view"]]);
    expect((await s.audiences(item, "maya")).audiences).toEqual([]); // already granted
    expect((await s.link_copy(item, link.grant_id)).url).toMatch(/\/s\/fx\d+/);
    await s.link_update(item, link.grant_id, undefined, 5);
    expect((await s.list(item)).grants[1]!.expires_at).toBe(5);
    await s.revoke(item, link.grant_id);
    l = await s.list(item);
    expect(l.grants).toHaveLength(1);
  });
  it("a secret that does not work is not_found", async () => { await expect(new FixtureSharing().guest_read("nope")).rejects.toMatchObject({ code: "not_found" }); });
});

describe("the QR encoder", () => {
  it("draws a link and refuses text that is too long", () => {
    expect(qrMatrix("https://calternal.example/s/abcdef")?.length).toBeGreaterThanOrEqual(21);
    expect(qrSvg("x".repeat(300))).toBe("");
    expect(qrSvg("https://calternal.example/s/abcdef")).toContain("<svg");
  });
});
