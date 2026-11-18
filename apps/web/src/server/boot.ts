/**
 * What the app does before it paints in server mode, in one place and only then (fixtures never load this file). The shell paints at once from what this device already knows (cache.ts: the account, whether it
 * is the operator, the spaces), and the server's answer confirms it a round trip later; a first visit, a signed-out device and an old record wait for the server instead, so a signed-out person never sees the
 * app and a signed-in person never sees the sign-in screen. The checks run in parallel, not one after another: the session (`auth/status`), the protocol (`describe`), who this is and what they may do
 * (`auth/me`), the spaces (`spaces.list`). Each has its own failure: no session → the sign-in or first-owner screen; another protocol → a plain notice; an unreachable server on a first visit → a screen with
 * Try again; on a known device the app starts anyway and the reconnecting banner (link.svelte.ts) tells the truth.
 */
import { applyCapabilities } from "../shell/capabilities.svelte";
import { spaces } from "../shell/spaces.svelte";
import { account, status } from "./auth";
import { clearCache, readCache, writeCache } from "./cache";
import { inviteSecret, redeemInvite } from "./people";
import { devToken, server } from "./client";
import { link } from "./link.svelte";
import { load as savedOnboarding, reconcile, save as saveOnboarding, clear as clearOnboarding } from "../shell/onboarding/flow";
import { dropWorkers, markWorkersCleaned, originWiped, wipeOrigin, workersCleaned } from "./origin-state";

/** The protocol this build speaks (`PROTOCOL_VERSION` in crates/core, in `describe`). A different number on the server means the two cannot be trusted to agree. */
export const EXPECTED_PROTOCOL = 1;

/** Where start-up goes next. */
export type Boot = { screen: "app" } | { screen: "signin"; needsSetup: boolean; invite?: string } | { screen: "onboarding" } | { screen: "offline" } | { screen: "version"; server: number; expected: number };

async function check(): Promise<Boot> {
  link.quiet = true;
  try { return await run(); } finally { link.quiet = false; }
}
/** Never settles: the page is about to load again. */
const reloading = (): Promise<never> => { location.reload(); return new Promise<never>(() => {}); };

async function run(): Promise<Boot> {
  // A service worker or a cache on this origin is not ours (nothing here registers one): remove them and load again from the network, once per tab.
  if (!workersCleaned() && (await dropWorkers())) { markWorkersCleaned(); return reloading(); }
  const [st, d, me] = await Promise.all([devToken() ? Promise.resolve(null) : status().catch(() => undefined), server.describe().catch(() => null), account.me().catch(() => null)]);
  if (st === undefined) return { screen: "offline" };
  const invite = inviteSecret(location.pathname) ?? undefined; // an invitation link: …/invites/<secret>
  // No owner exists: nothing stored on this origin can belong to this installation (an old account, an old onboarding record, an old cache): wipe it all, once, and start from the server.
  if (st && !st.signed_in && st.setup && !originWiped()) { await wipeOrigin(); return reloading(); }
  if (st && !st.signed_in) { clearCache(); return { screen: "signin", needsSetup: st.setup, invite }; }
  if (d && d.protocol_version !== EXPECTED_PROTOCOL) return { screen: "version", server: d.protocol_version, expected: EXPECTED_PROTOCOL };
  if (me) {
    const cached = readCache();
    applyCapabilities(me.user.capabilities);
    if (cached && cached.account !== me.user.id) { await wipeOrigin(); return reloading(); } // another person or another installation on this device: nothing stored here is theirs, start again from the server's answer
    // Part-way through the onboarding when the page reloaded or the tab closed: resume at the same step. A record of another account is another installation's and is dropped.
    const raw = savedOnboarding(), mine = reconcile(raw, me.user.id);
    if (raw && !mine) clearOnboarding();
    if (mine) { saveOnboarding(mine); return { screen: "onboarding" }; }
    if (invite) await redeem(invite);
    await spaces.load().catch(() => {});
    writeCache({ account: me.user.id, capabilities: me.user.capabilities, spaces: spaces.list });
  } else await spaces.load().catch(() => {});
  return { screen: "app" };
}

/** A signed-in person opened an invitation link: join the space, then show the app at its start (the secret leaves the address). The new space becomes the current one. */
async function redeem(secret: string): Promise<void> {
  try {
    const { space_id } = await redeemInvite(secret);
    history.replaceState(null, "", import.meta.env.BASE_URL);
    await spaces.load().catch(() => {});
    spaces.switchTo(space_id);
  } catch { history.replaceState(null, "", import.meta.env.BASE_URL); } // a used or unknown link: the app opens as usual (the invitation page of the server says why)
}

/**
 * Starts the checks. `paintNow` says the device's record is fresh enough to paint the app before the answer (the record has already been applied to the shared state); `result` is the server's answer, which
 * for a painted app may still turn the screen into the sign-in, version or offline screen.
 */
export function start(): { paintNow: boolean; result: Promise<Boot> } {
  // A device part-way through the onboarding never paints the app first: the server's answer decides, and the onboarding resumes.
  const cached = devToken() || savedOnboarding() ? null : readCache();
  if (cached) {
    spaces.setList(cached.spaces);
    applyCapabilities(cached.capabilities);
  }
  const result = check();
  // Painted from the record: nothing may be changed until the server has confirmed the session (link.svelte.ts).
  if (cached) link.holdWrites(result.then((b) => b.screen === "app" || b.screen === "offline"));
  return { paintNow: !!cached, result };
}
