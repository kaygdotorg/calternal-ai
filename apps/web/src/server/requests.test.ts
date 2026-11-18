import { describe, expect, it, vi } from "vitest";
import { answerInput, requestOf, ServerRequests, type WireRequest } from "./requests";
import { ServerError, type ServerClient } from "./client";

const wire: WireRequest = { id: "r1", space_id: "s1", thread_id: "t1", detail: { kind: "provider_sign_in", provider_id: "claude", label: "Claude needs you to sign in again", url: "https://example.com/in", display_code: null, needs_code: true, expires_at_ms: 99 } };

describe("provider sign-in on the wire", () => {
  it("reads a provider_sign_in request with nullable url and code", () => {
    expect(requestOf(wire)).toEqual({ id: "r1", kind: "signin", threadId: "t1", providerId: "claude", label: "Claude needs you to sign in again", url: "https://example.com/in", displayCode: null, needsCode: true, expiresAt: 99 });
  });
  it("answers with provider.sign_in.answer", () => {
    expect(answerInput("r1", { kind: "signin", code: "abc" })).toEqual({ operation: "provider.sign_in.answer", input: { id: "r1", code: "abc" } });
  });
  it("maps the status, and not_found to gone", async () => {
    const operate = vi.fn().mockResolvedValueOnce({ id: "r1", status: "done" }).mockRejectedValueOnce(new ServerError("not_found", "gone", 404));
    const r = new ServerRequests({ operate } as unknown as ServerClient, () => "s1", async () => {});
    expect(await r.signInStatus("r1")).toBe("done");
    expect(operate).toHaveBeenCalledWith("provider.sign_in.status", { id: "r1" });
    expect(await r.signInStatus("r1")).toBe("gone");
  });
  it("typing asks with the space and thread, and never throws", async () => {
    const operate = vi.fn().mockRejectedValue(new Error("away"));
    const r = new ServerRequests({ operate } as unknown as ServerClient, () => "s1", async () => {});
    await expect(r.signInTyping("t1")).resolves.toBeUndefined();
    expect(operate).toHaveBeenCalledWith("provider.sign_in.typing", { space_id: "s1", thread_id: "t1" });
  });
});
