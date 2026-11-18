import { describe, expect, test, vi } from "vitest";
import { account, proof } from "./auth";
import { server, ServerError } from "./client";
import * as m from "./mutations";
import * as people from "./people";

// One check per screen: every change Settings and onboarding make that the server may ask a fresh passkey proof for (passkey and device management in the account page, sign out of a device, admin people and groups,
// members and invitations, machine revoke/pause/mode/enroll, plugins, leaving or deleting a space) re-verifies once on `passkey_reverification_required`, tries again, and never reacts to `forbidden`.
const stale = () => new ServerError("passkey_reverification_required", "Verify with your passkey to continue.", 403);
const real = { operate: server.operate, json: server.json };
function arm(code: string) {
  let first = true; const calls: string[] = [];
  const answer = async (name: string) => { calls.push(name); if (first) { first = false; throw code === "stale" ? stale() : new ServerError(code, "Not yours.", 403); } return { code: "c", expires_ms: 1, script: "s" }; };
  server.operate = ((n: string) => answer(n)) as typeof server.operate;
  server.json = ((p: string) => answer(p)) as typeof server.json;
  return calls;
}
const screens: [string, () => Promise<unknown>][] = [
  ["account: rename a passkey", () => account.rename("p", "Phone")],
  ["account: remove a passkey", () => account.remove("p")],
  ["account: sign a device out", () => account.revokeInstallation("i")],
  ["members: add a person", () => people.addMember("s", { account_id: "a" }, "editor")],
  ["members: change a role", () => people.setRole("s", "a", "reader")],
  ["members: remove a person", () => people.removeMember("s", "a")],
  ["space: transfer it", () => people.transfer("s", "a")],
  ["admin people: disable an account", () => people.disable("a")],
  ["admin groups: create", () => people.createGroup("g")],
  ["admin groups: rename", () => people.renameGroup("g", "h")],
  ["admin groups: delete", () => people.deleteGroup("g")],
  ["admin groups: add a person", () => people.addToGroup("g", "a")],
  ["admin groups: remove a person", () => people.removeFromGroup("g", "a")],
  ["invites: create", () => people.createInvite("s", "editor")],
  ["invites: revoke", () => people.revokeInvite("s", "i")],
  ["machines: revoke", () => m.revokeMachine("m")],
  ["machines: pause or resume", () => m.drainMachine("m", true)],
  ["machines: sandboxed or direct (Settings and onboarding)", () => m.setDirect("m", true)],
  ["machines: make an enrollment code", () => m.enrollmentCode()],
  ["onboarding: the install script", () => m.installScript()],
  ["plugins: turn one on", () => m.setPlugin("s", "p", true)],
  ["space: leave", () => m.leaveSpace("s")],
  ["space: delete", () => m.deleteSpace("s")],
];
describe("every change that can demand a fresh proof asks once and retries once", () => {
  for (const [name, run] of screens) {
    test(name, async () => {
      const again = vi.fn().mockResolvedValue(undefined); proof.again = again; const calls = arm("stale");
      try { await run(); } finally { server.operate = real.operate; server.json = real.json; }
      expect(again).toHaveBeenCalledTimes(1);
      expect(calls).toHaveLength(2);
    });
    test(`${name}: a plain refusal (forbidden) shows the server's words and does not re-verify`, async () => {
      const again = vi.fn().mockResolvedValue(undefined); proof.again = again; arm("forbidden");
      try { await expect(run()).rejects.toThrow("Not yours."); } finally { server.operate = real.operate; server.json = real.json; }
      expect(again).not.toHaveBeenCalled();
    });
  }
});
