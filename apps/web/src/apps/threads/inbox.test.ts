import { describe, expect, it } from "vitest";
import { inboxOf, reasonLine } from "./inbox";
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
