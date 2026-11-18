import { describe, expect, it } from "vitest";
import { inboxOf, reasonLine, snoozeEnd, snoozedOf } from "./inbox";
import type { Request, ThreadSummary } from "./model";

const t = (id: string, over: Partial<ThreadSummary> = {}): ThreadSummary =>
  ({ id, spaceId: "s", title: id, parentId: null, state: "idle", workspace: "", updatedAt: 1, provider: "claude", model: "default", permission: "ask", runsOn: "machine", messageCount: 0, ...over }) as ThreadSummary;
const req = (threadId: string, kind: Request["kind"]) => ({ id: `${threadId}-${kind}`, threadId, kind }) as unknown as Request;

describe("inboxOf", () => {
  it("lists only threads that wait on the person, newest first, with each reason once", () => {
    const threads = [
      t("quiet"), t("running", { state: "running" }),
      t("old", { updatedAt: 1 }), t("new", { updatedAt: 9, state: "failed" }),
      t("both", { updatedAt: 5 }),
    ];
    const got = inboxOf(threads, [req("old", "question"), req("both", "approval"), req("both", "question"), req("both", "question")]);
    expect(got.map((i) => i.thread.id)).toEqual(["new", "both", "old"]);
    expect(got[1]!.reasons).toEqual(["Question", "Approval"]);
    expect(got[0]!.reasons).toEqual(["Failed"]);
  });
  it("is empty when nothing waits", () => expect(inboxOf([t("a"), t("b", { state: "running" })], [])).toEqual([]));
  it("words the reasons for a row", () => {
    expect(reasonLine(["Question", "Approval"])).toBe("Question and approval");
    expect(reasonLine(["Failed"])).toBe("Failed");
  });
});

describe("snoozing", () => {
  const now = 1_000_000;
  it("a snoozed thread leaves the Inbox until its time and is listed as snoozed, soonest first", () => {
    const threads = [t("a", { state: "failed", snoozedUntil: now + 5000 }), t("b", { state: "failed" }), t("c", { state: "failed", snoozedUntil: now + 1000 })];
    expect(inboxOf(threads, [], now).map((i) => i.thread.id)).toEqual(["b"]);
    expect(snoozedOf(threads, now).map((s) => s.thread.id)).toEqual(["c", "a"]);
    expect(inboxOf(threads, [], now + 6000).map((i) => i.thread.id).sort()).toEqual(["a", "b", "c"]);
  });
  it("a thread waiting for a subscription limit to reset is snoozed without any action, until the reset time", () => {
    const waiting = t("w", { state: "failed", notice: { kind: "retrying", rule: "subscription_reset", nextAt: now + 90_000 } as ThreadSummary["notice"] });
    expect(snoozeEnd(waiting, now)).toEqual({ until: now + 90_000, auto: true });
    expect(inboxOf([waiting], [], now)).toEqual([]);
    expect(snoozedOf([waiting], now)[0]).toMatchObject({ auto: true });
    expect(snoozeEnd(waiting, now + 100_000)).toBeUndefined();
  });
  it("other retry rules do not snooze", () => {
    const r = t("r", { notice: { kind: "retrying", rule: "provider_backoff", nextAt: now + 1000 } as ThreadSummary["notice"] });
    expect(snoozeEnd(r, now)).toBeUndefined();
  });
});
