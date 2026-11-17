/**
 * What the signed-in person may do, as far as the shell shows it. Fixtures: the person is the operator of the fixture server (holds the server-admin capability) unless the address says
 * `?as=member` (remembered for this browser; `?as=operator` puts it back), so both cases can be looked at. The real answer comes from the server's capability set for the caller.
 */
import { serverMode } from "../server/client";

const KEY = "cal.role";

function read(): "operator" | "member" {
  try {
    const q = new URLSearchParams(location.search).get("as");
    if (q === "member" || q === "operator") localStorage.setItem(KEY, q);
    return localStorage.getItem(KEY) === "member" ? "member" : "operator";
  } catch {
    return "operator";
  }
}

/** The capabilities the shell checks (reactive: the server's answer can arrive after the first paint). With the real server this starts false and start-up sets it from the account's capabilities (server/boot.ts). */
export const caps = $state({ serverAdmin: serverMode() ? false : read() === "operator", peopleManage: false, groupsManage: false });

/** Takes the account's capability names (`auth/me`) into the shell's checks. */
export function applyCapabilities(names: string[]): void {
  caps.serverAdmin = names.includes("server.settings");
  caps.peopleManage = names.includes("people.manage");
  caps.groupsManage = names.includes("groups.manage");
}
