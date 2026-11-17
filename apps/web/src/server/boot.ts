/**
 * What the app does before it paints in server mode, in one place and only then (fixtures never load this file). The shell paints at once from what this device already knows (cache.ts: the account, whether it
 * is the operator, the spaces), and the server's answer confirms it a round trip later; a first visit, a signed-out device and an old record wait for the server instead, so a signed-out person never sees the
 * app and a signed-in person never sees the sign-in screen. The checks run in parallel, not one after another: the session (`auth/status`), the protocol (`describe`), who this is and what they may do
 * (`auth/me`), the spaces (`spaces.list`). Each has its own failure: no session → the sign-in or first-owner screen; another protocol → a plain notice; an unreachable server on a first visit → a screen with
 * Try again; on a known device the app starts anyway and the reconnecting banner (link.svelte.ts) tells the truth.
 */
import { caps } from "../shell/capabilities.svelte";
import { spaces } from "../shell/spaces.svelte";
import { account, status } from "./auth";
import { clearCache, readCache, writeCache } from "./cache";
import { devToken, server } from "./client";
import { link } from "./link.svelte";

/** The protocol this build speaks (`PROTOCOL_VERSION` in crates/core, in `describe`). A different number on the server means the two cannot be trusted to agree. */
export const EXPECTED_PROTOCOL = 1;

/** Where start-up goes next. */
export type Boot = { screen: "app" } | { screen: "signin"; needsSetup: boolean } | { screen: "offline" } | { screen: "version"; server: number; expected: number };

async function check(): Promise<Boot> {
  link.quiet = true;
  try { return await run(); } finally { link.quiet = false; }
}
async function run(): Promise<Boot> {
  const [st, d, me] = await Promise.all([devToken() ? Promise.resolve(null) : status().catch(() => undefined), server.describe().catch(() => null), account.me().catch(() => null)]);
  if (st === undefined) return { screen: "offline" };
  if (st && !st.signed_in) { clearCache(); return { screen: "signin", needsSetup: st.setup }; }
  if (d && d.protocol_version !== EXPECTED_PROTOCOL) return { screen: "version", server: d.protocol_version, expected: EXPECTED_PROTOCOL };
  if (me) {
    const cached = readCache();
    caps.serverAdmin = me.user.capabilities.includes("server.settings");
    if (cached && cached.account !== me.user.id) { clearCache(); location.reload(); return { screen: "app" }; } // another person on this device: start again from the server's answer
    await spaces.load().catch(() => {});
    writeCache({ account: me.user.id, admin: caps.serverAdmin, spaces: spaces.list.map((s) => ({ id: s.id, name: s.name })) });
  } else await spaces.load().catch(() => {});
  return { screen: "app" };
}

/**
 * Starts the checks. `paintNow` says the device's record is fresh enough to paint the app before the answer (the record has already been applied to the shared state); `result` is the server's answer, which
 * for a painted app may still turn the screen into the sign-in, version or offline screen.
 */
export function start(): { paintNow: boolean; result: Promise<Boot> } {
  const cached = devToken() ? null : readCache();
  if (cached) {
    spaces.setList(cached.spaces);
    caps.serverAdmin = cached.admin;
  }
  return { paintNow: !!cached, result: check() };
}
