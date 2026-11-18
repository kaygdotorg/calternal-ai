/**
 * Browser sign-in against the real server: passkey ceremonies (`navigator.credentials.create/get`) over `/api/v1/auth/*`, and the session the server keeps in
 * its Secure, HttpOnly cookie (this code never sees the cookie; a request on the same origin carries it). Shapes are the server's (crates/server/src/account_http.rs):
 * `status` → `{setup, signed_in}`; `setup/start {token, name}` and `login/start {}` → `{ceremony, options}` (webauthn-rs, `options.publicKey`, binary fields base64url);
 * `.../finish {ceremony, credential, device}` → `{user:{id}}` plus the cookie; `logout` → 204. A failure is `{ok:false, error:{code,message}}`.
 */

/** The server's answer to `GET /api/v1/auth/status`. */
export interface AuthStatus { setup: boolean; signed_in: boolean }

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

async function ceremony(kind: "create" | "get", options: Json): Promise<PublicKeyCredential> {
  const d = decodeOptions(options);
  try {
    const c = (kind === "create" ? await navigator.credentials.create({ publicKey: d.create! }) : await navigator.credentials.get({ publicKey: d.get! })) as PublicKeyCredential | null;
    if (!c) throw new ServerError("cancelled", "No passkey was chosen.");
    return c;
  } catch (e) {
    if (e instanceof ServerError) throw e;
    const name = (e as Error).name;
    if (name === "NotAllowedError" || name === "AbortError") throw new ServerError("cancelled", "The passkey prompt was closed or timed out. Try again.");
    if (name === "InvalidStateError") throw new ServerError("passkey_failed", "This device already has a passkey here. Sign in instead.");
    throw new ServerError("passkey_failed", "This browser could not use a passkey.");
  }
}

/** A name for the installation row of this browser ("Chrome on Mac"), sent as `device`. */
export function deviceName(): string {
  const ua = navigator.userAgent;
  const b = /Edg\//.test(ua) ? "Edge" : /Firefox\//.test(ua) ? "Firefox" : /Chrome\//.test(ua) ? "Chrome" : /Safari\//.test(ua) ? "Safari" : "Browser";
  const o = /iPhone|iPad/.test(ua) ? "iOS" : /Android/.test(ua) ? "Android" : /Mac/.test(ua) ? "Mac" : /Windows/.test(ua) ? "Windows" : /Linux/.test(ua) ? "Linux" : "";
  return o ? `${b} on ${o}` : b;
}

/** First-owner setup: the one-use setup token and the owner's name; creates the passkey and signs in (the server sets the cookie). */
export async function setup(token: string, name: string): Promise<void> {
  const s = await post("setup/start", { token: token.trim(), name: name.trim() });
  const cred = await ceremony("create", s.options as Json);
  await post("setup/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), device: deviceName() });
}

/** Sign-in with a discoverable passkey (no account name is asked). */
export async function signIn(): Promise<void> {
  const s = await post("login/start", {});
  const cred = await ceremony("get", s.options as Json);
  await post("login/finish", { ceremony: s.ceremony, credential: encodeCredential(cred), device: deviceName() });
}

/** Ends this browser's session on the server. */
export async function signOut(): Promise<void> { clearCache(); await post("logout"); }

/** Who is signed in: the account (`name`, `role`, `capabilities`) and this browser's installation id. */
export interface Me { user: { id: string; name: string; username: string; role: string; capabilities: string[] }; installation: string }
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
/** Runs a change that may ask for a fresh passkey: if the server says so, verifies once and tries again. */
async function fresh<T>(f: () => Promise<T>): Promise<T> {
  try { return await f(); } catch (e) {
    if (e instanceof ServerError && e.code === "passkey_reverification_required") { await reverify(); return f(); }
    throw e;
  }
}
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
