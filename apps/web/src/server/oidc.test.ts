import { describe, expect, it, vi, beforeEach } from "vitest";
import { offered, providers } from "./oidc";
import { server } from "./client";

describe("oidc providers", () => {
  beforeEach(() => vi.restoreAllMocks());
  it("hides Apple and Google by id or preset", () => {
    expect(offered({ id: "acme" })).toBe(true);
    expect(offered({ id: "Google" })).toBe(false);
    expect(offered({ id: "work", preset: "apple" })).toBe(false);
  });
  it("lists only generic enabled providers, and nothing when the server fails", async () => {
    const spy = vi.spyOn(server, "json").mockResolvedValueOnce({ providers: [{ id: "acme", label: "Acme" }, { id: "google", label: "Google" }] });
    expect((await providers()).map((p) => p.id)).toEqual(["acme"]);
    spy.mockRejectedValueOnce(new Error("404"));
    expect(await providers()).toEqual([]);
  });
});
