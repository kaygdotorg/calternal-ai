/**
 * Sign-in with an outside identity provider (core OpenID Connect), over `/api/v1/auth/oidc/*` (docs/account-onboarding.md, "Core OpenID Connect").
 * Off by default on the server: when it is off `providers()` answers an empty list and no button shows. Only generic providers are offered in this
 * release; Apple and Google presets stay hidden even if a server lists them (`offered`).
 */
import { deviceName } from "./auth";
import { server } from "./client";

type Json = Record<string, unknown>;
/** A provider the server has enabled, as the sign-in screen shows it. */
export interface Provider { id: string; label: string; icon?: string }
/** A provider as the admin sees it (configured, maybe not enabled). */
export interface AdminProvider { id: string; preset?: string; label: string; issuer: string; client_id: string; scopes?: string[]; display_name_claim?: string; enabled: boolean }
/** What the admin enters to configure a provider; the client secret is sent once and never read back. */
export interface ProviderConfig { id: string; label: string; issuer: string; client_id: string; client_secret: string; scopes?: string[]; display_name_claim?: string }
/** Why a flow starts: sign in, link to the signed-in account, or confirm it is still you. */
export type Purpose = "sign_in" | "link" | "reverify";
/** The answer to finishing a flow: a new session (with what onboarding needs), or a link made. */
export interface Finished { linked?: boolean; user?: { id: string }; offer_passkey?: boolean; recovery_codes?: string[] }

const post = <T = Json>(path: string, body: Json = {}, link = false): Promise<T> => server.json<T>(`/api/v1/auth/oidc/${path}`, { body, link });
const FLOW = "cal.oidc.flow";
/** The ids the first release keeps hidden: they need vendor-specific handling that is not built. */
const HIDDEN = new Set(["apple", "google"]);

/** Whether a listed provider may be shown: generic only (neither its id nor its preset is Apple or Google). */
export const offered = (p: { id: string; preset?: string }): boolean => !HIDDEN.has(p.id.toLowerCase()) && !HIDDEN.has((p.preset ?? "").toLowerCase());

/** The enabled providers, generic only. Any failure (an older server, OIDC off) is an empty list: the sign-in screen then shows only the passkey. */
export async function providers(): Promise<Provider[]> {
  try { return (await server.json<{ providers: Provider[] }>("/api/v1/auth/oidc/providers", { link: false })).providers.filter(offered); }
  catch { return []; }
}

/** Starts a flow and leaves this page for the provider. The flow id is kept in this tab (it must survive the round trip) and `finishPending` reads it back. */
export async function start(providerId: string, purpose: Purpose = "sign_in", signup = false): Promise<void> {
  const r = await post<{ flow_id: string; redirect_url: string }>("start", { provider_id: providerId, purpose, ...(signup ? { signup: true } : {}) }, purpose !== "sign_in");
  sessionStorage.setItem(FLOW, JSON.stringify({ id: r.flow_id, purpose }));
  location.assign(r.redirect_url);
}

/** Whether this page load is the provider's return (`?account-oidc=complete`) with a flow this tab started. */
export const returning = (): boolean => new URLSearchParams(location.search).get("account-oidc") === "complete" && !!sessionStorage.getItem(FLOW);

/** Finishes the flow this tab started, if any, and cleans the address. Returns null when there is nothing to finish. Throws the server's refusal (shown by the caller). */
export async function finishPending(): Promise<(Finished & { purpose: Purpose }) | null> {
  const raw = sessionStorage.getItem(FLOW);
  const u = new URL(location.href);
  const had = u.searchParams.has("account-oidc");
  u.searchParams.delete("account-oidc");
  if (had) history.replaceState(null, "", u.pathname + u.search + u.hash);
  if (!raw || !had) return null;
  sessionStorage.removeItem(FLOW);
  const f = JSON.parse(raw) as { id: string; purpose: Purpose };
  return { ...(await post<Finished>("finish", { flow_id: f.id, device: deviceName() }, f.purpose !== "sign_in")), purpose: f.purpose };
}

/** Removes a linked provider from this account. */
export const unlink = (providerId: string): Promise<Json> => post("unlink", { provider_id: providerId }, true);

/** Admin: every configured provider (enabled or not), generic only. */
export async function adminProviders(): Promise<AdminProvider[]> {
  return (await server.json<{ providers: AdminProvider[] }>("/api/v1/auth/oidc/admin/providers")).providers.filter(offered);
}
/** Admin: adds or replaces a provider's configuration. It stays disabled until `setEnabled`. */
export const saveProvider = (c: ProviderConfig): Promise<Json> => post("admin/providers", { ...c }, true);
/** Admin: turns a provider on or off for sign-in. `operatorRecovery` is the server's acknowledgement when disabling the last way someone signs in. */
export const setEnabled = (providerId: string, on: boolean, operatorRecovery = false): Promise<Json> =>
  post("admin/state", { provider_id: providerId, action: on ? "enable" : "disable", ...(operatorRecovery ? { operator_recovery: true } : {}) }, true);
