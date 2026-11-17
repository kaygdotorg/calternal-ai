import { describe, expect, test } from "vitest";
import { server } from "./client";
import * as people from "./people";

// The shapes are the recorded ones from the people lane's contract (docs/reviews/people-over-http-2026-11-17.md): a fake `operate` answers each operation with its recorded output.
const calls: { op: string; input: Record<string, unknown> }[] = [];
const answers: Record<string, (i: Record<string, unknown>) => unknown> = {};
server.operate = (async (op: string, input: Record<string, unknown> = {}) => { calls.push({ op, input }); return answers[op]!(input); }) as typeof server.operate;
const reset = () => { calls.length = 0; for (const k of Object.keys(answers)) delete answers[k]; };

describe("people adapter against recorded shapes", () => {
  test("space grants: names, roles and groups, following the cursor", async () => {
    reset();
    answers["spaces.members"] = (i) => i.cursor
      ? { members: [{ account_id: "b", name: "Bea", role: "editor" }], groups: [{ group_id: "g", name: "Design", member_count: 3, role: "reader" }], next_cursor: null }
      : { members: [{ account_id: "a", name: "Ana", role: "owner" }], groups: [{ group_id: "g", name: "Design", member_count: 3, role: "reader" }], next_cursor: "c1" };
    const g = await people.grants("s1");
    expect(g.members.map((m) => m.name)).toEqual(["Ana", "Bea"]);
    expect(g.groups).toHaveLength(1);
    expect(calls.map((c) => c.input.cursor)).toEqual([undefined, "c1"]);
  });
  test("granting names exactly one of person or group, and never sends owner", async () => {
    reset(); answers["spaces.add_member"] = () => ({});
    await people.addMember("s1", { account_id: "a" }, "editor");
    await people.addMember("s1", { group_id: "g" }, "reader");
    expect(calls[0]!.input).toEqual({ space_id: "s1", role: "editor", account_id: "a" });
    expect(calls[1]!.input).toEqual({ space_id: "s1", role: "reader", group_id: "g" });
  });
  test("an invitation without an expiry leaves the choice to the server; with one it is sent in milliseconds", async () => {
    reset(); answers["invites.create"] = () => ({ invite_id: "i", expires_at: 1 });
    await people.createInvite("s1", "editor");
    await people.createInvite("s1", "reader", 1_700_000_000_000);
    expect(calls[0]!.input).toEqual({ space_id: "s1", role: "editor" });
    expect(calls[1]!.input).toEqual({ space_id: "s1", role: "reader", expires_at: 1_700_000_000_000 });
  });
  test("lists of people and groups are followed to the end, and the invite list keeps live links", async () => {
    reset();
    answers["people.list"] = (i) => (i.cursor ? { people: [{ account_id: "2", name: "B", role: "member", disabled: false }], next_cursor: null } : { people: [{ account_id: "1", name: "A", role: "admin", disabled: false }], next_cursor: "n" });
    expect((await people.people()).map((p) => p.account_id)).toEqual(["1", "2"]);
    answers["invites.list"] = () => ({ invites: [{ invite_id: "i", role: "editor", created_at: 1, expires_at: 2, used_at: null, revoked_at: null, link: "https://x/invites/abc" }], next_cursor: null });
    expect((await people.invites("s1"))[0]!.link).toBe("https://x/invites/abc");
  });
  test("the secret of an invitation link, under the root or under the app's mount", () => {
    expect(people.inviteSecret("/invites/abc-123")).toBe("abc-123");
    expect(people.inviteSecret("https://preview.calternal.cloud/ai/invites/abc-123?x=1")).toBe("abc-123");
    expect(people.inviteSecret("/notes/n-1")).toBeNull();
  });
});
