/**
 * What the app does before it paints in server mode, in one place and only then (fixtures never load this file): is there a session (a cookie session asks `auth/status`; a development token
 * is trusted until the server says otherwise), do the server and this build speak the same protocol (`describe`), who is this (`auth/me`: the real capability set, so Admin shows only for the
 * operator), and which spaces are there. Each step has its own failure: no session → the sign-in or first-owner screen; another protocol → a plain notice; a slow or unreachable server → the
 * app starts anyway and the reconnecting banner (link.svelte.ts) tells the truth.
 */
import { caps } from "../shell/capabilities";
import { spaces } from "../shell/spaces.svelte";
import { account, status } from "./auth";
import { devToken, server } from "./client";

/** The protocol this build speaks (`PROTOCOL_VERSION` in crates/core, in `describe`). A different number on the server means the two cannot be trusted to agree. */
export const EXPECTED_PROTOCOL = 1;

/** Where start-up goes next. */
export type Boot = { screen: "app" } | { screen: "signin"; needsSetup: boolean } | { screen: "offline" } | { screen: "version"; server: number; expected: number };

/** Decides the first screen; the shared state (capabilities, spaces) is filled in on the way. */
export async function boot(): Promise<Boot> {
  if (!devToken()) {
    const st = await status().catch(() => null);
    if (!st) return { screen: "offline" };
    if (!st.signed_in) return { screen: "signin", needsSetup: st.setup };
  }
  const d = await server.describe().catch(() => null);
  if (d && d.protocol_version !== EXPECTED_PROTOCOL) return { screen: "version", server: d.protocol_version, expected: EXPECTED_PROTOCOL };
  const me = await account.me().catch(() => null);
  if (me) caps.serverAdmin = me.user.capabilities.includes("server.settings");
  await Promise.race([spaces.load().catch(() => {}), new Promise((r) => setTimeout(r, 4000))]);
  return { screen: "app" };
}
