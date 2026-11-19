/**
 * Browser sign-in against the real server: passkey ceremonies (`navigator.credentials.create/get`) over `/api/v1/auth/*`, and the session the server keeps in
 * its Secure, HttpOnly cookie (this code never sees the cookie; a request on the same origin carries it). Shapes are the server's (crates/server/src/account_http.rs):
 * `status` → `{setup, signed_in}`; `setup/start {token, name}` and `login/start {}` → `{ceremony, options}` (webauthn-rs, `options.publicKey`, binary fields base64url);
 * `.../finish {ceremony, credential, device}` → `{user:{id}}` plus the cookie; `logout` → 204. A failure is `{ok:false, error:{code,message}}`.
 */

/** The server's answer to `GET /api/v1/auth/status`. */
export interface AuthStatus { setup: boolean; signed_in: boolean; /** The hosted service requires a verified email at signup; self-hosted may omit it. */ email_required?: boolean; /** The server can send mail (SMTP); without it an address can only be saved unverified. */ mail_available?: boolean }

const b64 = {
  /** base64url text → bytes. */
  toBytes(s: string): ArrayBuffer {
    const p = s.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(s.length / 4) * 4, "=");
    const bin = atob(p);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out.buffer;
  },
  /** bytes → base64url text (no padding). */
  fromBytes(b: ArrayBuffer | null | undefined): string | undefined {
    if (!b) return undefined;
    let s = "";
    for (const x of new Uint8Array(b)) s += String.fromCharCode(x);
    return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
  },
};
/** The base64url helpers WebAuthn needs (bytes to text and back, no padding); exported for tests and for the passkey flows in other modules. Malformed input throws from `atob`. */
export const base64url = b64;

import { clearCache } from "./cache";
import { server, ServerError } from "./client";

type Json = Record<string, unknown>;
interface Descriptor { id: string; type: string; transports?: string[] }

/** A POST to an account route. Ceremonies and sign-in stay out of the session-ended logic (`link: false`); changes after sign-in (`link: true`) and the reads (`get`) report like any other call. */
const post = (path: string, body: Json = {}, link = false): Promise<Json> => server.json(`/api/v1/auth/${path}`, { body, link });
const get = (path: string): Promise<Json> => server.json(`/api/v1/auth/${path}`);

/** Whether the installation still needs its first owner, and whether this browser has a session. */
export const status = (): Promise<AuthStatus> => server.json<AuthStatus>("/api/v1/auth/status", { link: false });

const descriptors = (l: Descriptor[] | undefined): PublicKeyCredentialDescriptor[] | undefined =>
  l?.map((d) => ({ ...d, id: b64.toBytes(d.id) }) as PublicKeyCredentialDescriptor);

/** The ceremony's `options.publicKey` with its base64url fields turned into bytes. */
function decodeOptions(o: Json): { create?: PublicKeyCredentialCreationOptions; get?: PublicKeyCredentialRequestOptions } {
  const pk = (o.publicKey ?? o) as Json & { challenge: string; user?: { id: string }; excludeCredentials?: Descriptor[]; allowCredentials?: Descriptor[] };
  const base = { ...pk, challenge: b64.toBytes(pk.challenge) } as Json;
  if (pk.user) return { create: { ...base, user: { ...pk.user, id: b64.toBytes(pk.user.id) }, excludeCredentials: descriptors(pk.excludeCredentials) } as unknown as PublicKeyCredentialCreationOptions };
  return { get: { ...base, allowCredentials: descriptors(pk.allowCredentials) } as unknown as PublicKeyCredentialRequestOptions };
}

/** What the server verifies: the credential as JSON, binary fields base64url (webauthn-rs's `RegisterPublicKeyCredential` / `PublicKeyCredential`). */
export function encodeCredential(c: PublicKeyCredential): Json {
  const r = c.response as AuthenticatorAttestationResponse & AuthenticatorAssertionResponse;
  const response: Json = { clientDataJSON: b64.fromBytes(r.clientDataJSON) };
  if ("attestationObject" in r && r.attestationObject) {
    response.attestationObject = b64.fromBytes(r.attestationObject);
    const t = (r as AuthenticatorAttestationResponse).getTransports?.();
    if (t?.length) response.transports = t;
  } else {
    response.authenticatorData = b64.fromBytes(r.authenticatorData);
    response.signature = b64.fromBytes(r.signature);
    if (r.userHandle) response.userHandle = b64.fromBytes(r.userHandle);
  }
  return { id: c.id, rawId: b64.fromBytes(c.rawId), type: c.type, response, extensions: c.getClientExtensionResults() };
}

/** A WebAuthn rejection that comes back sooner than this was not the person: no sheet had time to be shown, so the browser (or the app's web view) refused the request itself. */
export const REFUSED_AT_ONCE_MS = 400;
/**
 * What a rejected `navigator.credentials.create/get` means, by the error's name and how long it took (milliseconds from the call): `InvalidStateError` (a passkey for this account is already here),
 * `SecurityError` (the page's address cannot be used for passkeys), `NotSupportedError`, `NotAllowedError` at once (refused outright: in an app's web view with no site association there is no sheet to cancel), a
 * `NotAllowedError` or `AbortError` that took longer (the person closed the sheet, or it timed out), anything else. Pure, so the wording can be tested without a browser.
 */
export function passkeyFailure(name: string, elapsedMs: number): { code: string; message: string } {
  if (name === "InvalidStateError") return { code: "passkey_failed", message: "This device already has a passkey here. Sign in instead." };
  if (name === "SecurityError") return { code: "passkey_origin", message: "This page's address cannot be used for passkeys." };
  if (name === "NotSupportedError") return { code: "passkey_unsupported", message: "This device or browser does not support passkeys." };
  if (name === "NotAllowedError" && elapsedMs < REFUSED_AT_ONCE_MS) return { code: "passkey_unavailable", message: "Passkeys are not available in this app or browser." };
  if (name === "NotAllowedError" || name === "AbortError") return { code: "cancelled", message: "The passkey prompt was closed or timed out. Try again." };
  return { code: "passkey_failed", message: "This browser could not use a passkey." };
}

async function ceremony(kind: "create" | "get", options: Json): Promise<PublicKeyCredential> {
  const d = decodeOptions(options);
  const t0 = performance.now();
  try {
    const c = (kind === "create" ? await navigator.credentials.create({ publicKey: d.create! }) : await navigator.credentials.get({ publicKey: d.get! })) as PublicKeyCredential | null;
    if (!c) throw new ServerError("cancelled", "No passkey was chosen.");
    return c;
  } catch (e) {
    if (e instanceof ServerError) throw e;
    const f = passkeyFailure((e as Error).name, performance.now() - t0);
    throw new ServerError(f.code, f.message);
  }
}

/** A name for the installation row of this browser ("Chrome on Mac"), sent as `device`. */
export function deviceName(): string {
  const ua = navigator.userAgent;
  const b = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const o = /iPhone|iPad/.test(ua) ? "iOS" : /Android/.test(ua) ? "Android" : /Mac/.test(ua) ? "Mac" : /Windows/.test(ua) ? "Windows" : /Linux/.test(ua) ? "Linux" : "";
  return o ? `${b} on ${o}` : b;
}

/** What the server hands over once, when the account is made: ten recovery codes (shown once, never kept) and whether to prompt for a second passkey. */
export interface SignupResult { recovery_codes: string[]; second_passkey_prompt: boolean }
let signup: SignupResult | null = null;
/** The signup result, once: the recovery step takes it and it is gone (it is never stored). */
export function takeSignupResult(): SignupResult | null { const s = signup; signup = null; return s; }

/** The extra things setup sends: the username, and the email with the proof that it was confirmed. */
export interface SignupDetails { username: string; email?: string; verification_id?: string }

/** Account setup (the first owner's or an invited person's): the one-use token, the name, the username and optionally the email; creates the passkey and signs in (the server sets the cookie). The one-time answer waits in `takeSignupResult`. */
export async function setup(token: string, name: string, details?: SignupDetails): Promise<void> {
  const s = await post("setup/start", { token: token.trim(), name: name.trim(), ...(details ? { username: details.username.trim(), ...(details.email ? { email: details.email.trim() } : {}), ...(details.verification_id ? { verification_id: details.verification_id } : {}) } : {}) });
  const cred = await ceremony("create", s.options as Json);
  const fin = await post("setup/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), device: deviceName() });
  signup = Array.isArray(fin.recovery_codes) ? { recovery_codes: fin.recovery_codes as string[], second_passkey_prompt: !!fin.second_passkey_prompt } : null;
}

/** Sends an eight-digit code to the address (signup passes the setup or invite token; a signed-in change passes none). */
export const emailStart = (email: string, token?: string): Promise<{ challenge_id: string; expires_at: number }> => post("email/start", { email: email.trim(), ...(token ? { token: token.trim() } : {}) }) as Promise<{ challenge_id: string; expires_at: number }>;
/** Checks the code the person typed back; the answer carries the proof setup needs. */
export const emailConfirm = (challenge_id: string, code: string, token?: string): Promise<{ verification_id?: string; email_status: string }> => post("email/confirm", { challenge_id, code, ...(token ? { token: token.trim() } : {}) }) as Promise<{ verification_id?: string; email_status: string }>;
/** Makes a new set of recovery codes (the old set stops working); the new ones are returned once. */
export const regenerateRecoveryCodes = async (): Promise<string[]> => ((await fresh(() => post("recovery/codes/regenerate", {}, true))) as { recovery_codes: string[] }).recovery_codes;
/** Dismisses the prompt to add a second passkey (it returns in Settings, Account while there is one passkey). */
export const dismissPasskeyPrompt = (): Promise<unknown> => post("passkeys/prompt/dismiss", {});
/** Changes the username. */
export const changeUsername = (username: string): Promise<{ username: string }> => fresh(() => post("username/change", { username: username.trim() }, true)) as Promise<{ username: string }>;

/** Sign-in with a discoverable passkey (no account name is asked). */
export async function signIn(): Promise<void> {
  const s = await post("login/start", {});
  const cred = await ceremony("get", s.options as Json);
  await post("login/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), device: deviceName() });
}

/** Ends this browser's session on the server. */
export async function signOut(): Promise<void> { clearCache(); await post("logout"); }

/** Who is signed in: the account (`name`, `role`, `capabilities`) and this browser's installation id. */
export interface Me { user: { id: string; name: string; username: string; role: string; capabilities: string[] }; installation: string; email?: string | null; email_status?: "missing" | "unverified" | "verified"; mail_available?: boolean; second_passkey_prompt?: boolean; recovery_codes_configured?: boolean; /** The server's policy requires a verified email and this account has none: ask for one after accepting the session. */ email_setup_required?: boolean; /** The provider this session signed in with, if any. */ sign_in_provider?: string | null; /** Providers linked to this account. */ provider_links?: { provider_id: string; label?: string }[] }
/** A passkey as the server lists it. */
export interface Passkey { id: string; name: string; created_at: number; last_used_at: number | null }
/** A signed-in browser or app of this account. */
export interface InstallationRow { id: string; kind: string; name: string; created_at: number; last_seen_at: number; current: boolean }

/** Asks again for a passkey (`reverify/*`): the server wants a fresh one before it changes how someone signs in. */
export async function reverify(): Promise<void> {
  const s = await post("reverify/start", {});
  const cred = await ceremony("get", s.options as Json);
  await post("reverify/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), device: deviceName() });
}
/**
 * Runs a call that may need a fresh passkey proof (approving a request, providing a secret, changing how someone signs in): when the server answers HTTP 403 with the dedicated code `passkey_reverification_required`
 * (crates/types/src/error.rs; docs/requests.md), asks for the passkey once (`again`, the real ceremony by default; tests pass a stand-in) and tries the call again, now with the renewed session. Any other refusal, and a
 * second refusal, is thrown as the server said it (the person is shown its plain message). The ONE helper for every call that can demand a fresh proof: do not match on `forbidden`, which the server uses for other refusals.
 * Cost: none unless the proof is stale; then one passkey ceremony (two requests and the person's touch).
 */
export async function withFreshProof<T>(f: () => Promise<T>, again: () => Promise<void> = reverify): Promise<T> {
  try { return await f(); } catch (e) {
    if (e instanceof ServerError && e.code === "passkey_reverification_required") { await again(); return f(); }
    throw e;
  }
}
/** The re-verification every screen's changes use: the real passkey ceremony. Tests replace `proof.again` to answer it without a device. */
export const proof = { again: reverify };
/** `withFreshProof` with the shared ceremony: the one wrapper for a change that may demand a fresh passkey (docs/requests.md). */
export const fresh = <T>(call: () => Promise<T>): Promise<T> => withFreshProof(call, () => proof.again());
/** The account endpoints behind Settings, Account. */
export const account = {
  me: () => get("me") as unknown as Promise<Me>,
  passkeys: async () => ((await get("passkeys")).passkeys ?? []) as Passkey[],
  installations: async () => ((await get("installations")).installations ?? []) as InstallationRow[],
  rename: (id: string, name: string) => fresh(() => post("passkeys/rename", { id, name }, true)),
  remove: (id: string) => fresh(() => post("passkeys/remove", { id }, true)),
  revokeInstallation: (installation: string) => fresh(() => post("installations/revoke", { installation }, true)),
  /** Creates a passkey on this device and adds it to the account. */
  async addPasskey(name: string): Promise<void> {
    await fresh(async () => {
      const s = await post("passkeys/add/start", {});
      const cred = await ceremony("create", s.options as Json);
      await post("passkeys/add/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), name });
    });
  },
};
