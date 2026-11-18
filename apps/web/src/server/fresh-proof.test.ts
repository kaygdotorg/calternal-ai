import { describe, expect, it, vi } from "vitest";
import { ServerError } from "./client";
import { withFreshProof } from "./auth";

const stale = () => new ServerError("passkey_reverification_required", "Verify with your passkey to continue.", 403);

describe("withFreshProof", () => {
  it("passes a call that needs no proof straight through", async () => {
    const again = vi.fn();
    expect(await withFreshProof(async () => 7, again)).toBe(7);
    expect(again).not.toHaveBeenCalled();
  });
  it("asks for the passkey once on passkey_reverification_required and tries again (403, then the ceremony, then 200)", async () => {
    const call = vi.fn().mockRejectedValueOnce(stale()).mockResolvedValueOnce("approved");
    const again = vi.fn().mockResolvedValue(undefined);
    expect(await withFreshProof(call, again)).toBe("approved");
    expect(again).toHaveBeenCalledTimes(1);
    expect(call).toHaveBeenCalledTimes(2);
  });
  it("shows the server's own words when the second try is refused too, and asks only once", async () => {
    const call = vi.fn().mockRejectedValue(stale());
    const again = vi.fn().mockResolvedValue(undefined);
    await expect(withFreshProof(call, again)).rejects.toMatchObject({ message: "Verify with your passkey to continue." });
    expect(again).toHaveBeenCalledTimes(1);
  });
  it("does not run the ceremony for any other refusal, `forbidden` included", async () => {
    const again = vi.fn();
    await expect(withFreshProof(async () => { throw new ServerError("forbidden", "Not yours.", 403); }, again)).rejects.toThrow("Not yours.");
    expect(again).not.toHaveBeenCalled();
  });
});
