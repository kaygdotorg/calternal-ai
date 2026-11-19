/**
 * The real server's client: operations (`POST /api/v1/operations`, one call each with a fresh `request_id` that a retry keeps), the routes that are not operations (`json`: account, passkeys, machines), and the event
 * streams (`live`, `accountLive`, `jobEvents`). Types come from `packages/contracts` (generated from the Rust contract). Same origin: the page is served by the server (or its proxy), and in development the Vite
 * server proxies `/api` to the server on 127.0.0.1 (vite.config.ts, `CAL_SERVER`).
 *
 * A signed-in browser has the server's Secure, HttpOnly session cookie, which this code never sees; the DEVELOPMENT TOKEN MODE (a bearer token from `calternal serve --development`, given once as `?token=`, kept in
 * localStorage, never printed) is for tests and tools. Every failure is a `ServerError` with the server's stable code (or `unavailable` when it cannot be reached), and every call reports to the link state
 * (link.svelte.ts: reconnecting, session ended). Until the session is confirmed at start-up a change is held, never sent (see `link.holdWrites`).
 */
import type {
  AccountFrame, AccountInputListResult, AccountInputResult, ApiError, DescribeResult, LiveFrame, LiveSubscription, NoteResult, NotesCreateInput, NotesHistoryResult, NotesListResult, NotesUpdateInput, OperationResponse,
  SpacesListResult,
} from "../../../../packages/contracts/src/index";
import contractReads from "virtual:contract-reads";
import { events } from "./sse";
import { link } from "./link.svelte";

/** The operations that only read, from the contract itself (`mutates: false`, derived at build time): they are answered only for a live session, so they may go before the session is confirmed. A read held behind the session check can deadlock the start-up that waits for it (`onboarding.state` once did), so the set is never kept by hand. */
const READ_OPS = new Set<string>(contractReads);
/** Whether an operation only reads, so it may go before the session is confirmed. Everything else is held (default deny). */
export const isRead = (operation: string): boolean => READ_OPS.has(operation);

/** A failure the server reported (`code` is its stable category) or a transport failure (`code` is "unavailable"). */
export class ServerError extends Error {
  /** The server's stable category (`ApiError["code"]`), or one of the account routes' own (`passkey_failed`, `setup_token_invalid`, …) or this page's (`unavailable`, `cancelled`); say.ts words each. */
  constructor(public code: ApiError["code"] | (string & {}), message: string, public status = 0, /** True when the server itself answered with this code (its envelope), so `message` is the server's own words; false for a transport failure or a gateway's page, where nothing was reached. */ public answered = false) { super(message); }
}

/** The failure category of a response that carried no error body of its own. */
const codeOfStatus = (s: number): string => (s === 401 ? "unauthorized" : s === 403 ? "forbidden" : s === 404 ? "not_found" : s === 409 ? "conflict" : s === 413 ? "too_large" : s === 429 ? "too_many_requests" : s < 500 ? "invalid_input" : "internal");

/** How to build a `ServerClient`: `base` (default: same origin), `token` (the development bearer, or null for a cookie session) and an optional `fetch` (tests). */
export interface ServerOptions {
  base?: string; token: () => string | null; fetch?: typeof fetch;
  /** How an operation waits out the server being away (`unavailable`): the first pause, the longest pause and how long in all, in milliseconds. Tests shorten it. */
  retry?: { first: number; max: number; total: number };
}

/** Statuses a proxy or gateway answers with while the server behind it is away (an update, a restart). */
const TEMPORARY = new Set([502, 503, 504]);
/**
 * Whether a response is the server's own answer, whatever its status: it carries our JSON envelope (`{ok, ...}` for operations, `{error: {code, ...}}` for the account routes). A 503 like that is a refusal (no machine,
 * not ready) that repeating will not change, so it is returned at once; only an answer that is NOT our envelope (a gateway's HTML or text page) is the server being away.
 */
async function carriesEnvelope(res: Response): Promise<boolean> {
  try {
    const body: unknown = await res.clone().json();
    return !!body && typeof body === "object" && (typeof (body as { ok?: unknown }).ok === "boolean" || typeof ((body as { error?: { code?: unknown } }).error?.code) === "string");
  } catch { return false; }
}
/** A gateway's 502, 503 or 504 (not the server's own answer): the server is away. */
const gatewayDown = async (res: Response): Promise<boolean> => TEMPORARY.has(res.status) && !(await carriesEnvelope(res));
/** The default patience: 250 ms, doubling to 3 s between tries, for up to two minutes. */
const PATIENCE = { first: 250, max: 3000, total: 120_000 };

/** The server's client. */
export class ServerClient {
  private base: string;
  private f: typeof fetch;
  /** `fetch` without the link-state reporting (see `json`). */
  private plain: typeof fetch;
  private token: () => string | null;
  private retry: { first: number; max: number; total: number };
  constructor(o: ServerOptions) {
    this.base = o.base ?? "";
    this.retry = o.retry ?? PATIENCE;
    this.token = o.token;
    const raw = o.fetch ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
    this.plain = raw;
    // Every call reports to the shared link state (reconnecting banner, expired session); a call the caller cancelled is not a failure.
    this.f = (...a) => raw(...a).then(async (r) => { if (r.status === 401) link.expired(!!this.token()); else if (await gatewayDown(r)) link.failed(); else link.ok(); return r; }, (e: Error) => { if (e?.name !== "AbortError") link.failed(); throw e; });
  }
  /**
   * A call to a route that is not an operation (account, passkeys, machines): JSON in, JSON out (`{}` for 204), a failure as `ServerError` with the server's own code (or one from the status) and no server text
   * reaching the screen (say.ts words it). The one place for this plumbing. `link: false` keeps it out of the session-ended logic: a refused passkey is a 401 too, and must not look like a session that ended.
   */
  async json<T = Record<string, unknown>>(path: string, o: { method?: "GET" | "POST"; body?: unknown; link?: boolean } = {}): Promise<T> {
    const method = o.method ?? (o.body === undefined ? "GET" : "POST");
    const res = await this.send(path, { method, headers: o.body === undefined ? {} : { "content-type": "application/json" }, body: o.body === undefined ? undefined : JSON.stringify(o.body) }, o.link === false, method === "POST");
    if (res.status === 204) return {} as T;
    const data = (await res.json().catch(() => ({}))) as Record<string, unknown>;
    if (!res.ok) { const e = (data.error ?? {}) as { code?: string; message?: string }; throw new ServerError(e.code ?? codeOfStatus(res.status), e.message ?? "", res.status, !!e.code); }
    return data as T;
  }
  /** Every call goes through here: the session's headers, the link-state reporting (unless `plain`), and a transport failure as `ServerError("unavailable")`. */
  private async send(path: string, init: RequestInit = {}, plain = false, change = false): Promise<Response> {
    if (change && !plain) await link.mayWrite();
    let res: Response;
    try { res = await (plain ? this.plain : this.f)(`${this.base}${path}`, { ...init, headers: this.headers(init.headers as Record<string, string>) }); }
    catch { throw new ServerError("unavailable", "Can't reach the server."); }
    // A proxy's 502, 503 or 504 is the server being away, whatever its body says (an HTML page, not our envelope): the same as a dropped connection.
    if (await gatewayDown(res)) throw new ServerError("unavailable", "Can't reach the server.", res.status);
    return res;
  }
  /** An event stream (SSE): the body to read, or `ServerError` when the server refuses it. */
  private async stream(path: string, signal?: AbortSignal): Promise<ReadableStream<Uint8Array>> {
    const res = await this.send(path, { headers: { accept: "text/event-stream" }, signal });
    if (!res.ok || !res.body) throw new ServerError(res.status === 404 ? "not_found" : "unavailable", "", res.status);
    return res.body;
  }
  private headers(extra: Record<string, string> = {}): Record<string, string> {
    const t = this.token();
    return { ...(t ? { authorization: `Bearer ${t}` } : {}), ...extra };
  }
  /** Calls one operation. `requestId` makes a retry the same request. */
  async operate<T>(operation: string, input: Record<string, unknown> = {}, requestId: string = crypto.randomUUID()): Promise<T> {
    const init = { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ request_id: requestId, operation, input }) };
    // While the server is away (a dropped connection, or a proxy's 502, 503 or 504) the call waits and tries again with the SAME request id, backing off, until it answers or the patience runs out: the server's receipts
    // make a repeat of something it already did answer with the first result, so a retry is never a second change. The shell shows the reconnecting state meanwhile (`link`). Only the server being away is retried: its own refusal
    // (any answer with an envelope) goes to the caller at once.
    const { first, max, total } = this.retry;
    const until = Date.now() + total;
    let pause = first;
    let res: Response;
    for (;;) {
      try { res = await this.send("/api/v1/operations", init, false, !isRead(operation)); break; }
      catch (e) {
        if (!(e instanceof ServerError) || e.code !== "unavailable" || Date.now() + pause > until) throw e;
        await new Promise((r) => setTimeout(r, pause));
        pause = Math.min(pause * 2, max);
      }
    }
    let body: OperationResponse<T>;
    try { body = (await res.json()) as OperationResponse<T>; } catch { throw new ServerError("internal", `unreadable answer (${res.status})`, res.status); }
    if (!body.ok) throw new ServerError(body.error.code, body.error.message, res.status, true);
    return body.data;
  }
  /** What the server offers and who is asking (`GET /api/v1/describe`). */
  async describe(): Promise<DescribeResult> {
    const res = await this.send("/api/v1/describe");
    // Unlike an operation, discovery answers with the description itself (no envelope); a failure still arrives as a failure envelope.
    const body = (await res.json()) as DescribeResult | OperationResponse<never>;
    if ("ok" in body && body.ok === false) throw new ServerError(body.error.code, body.error.message, res.status, true);
    return body as DescribeResult;
  }
  /** A plain JSON POST to a route that is not an operation (the trusted secret endpoint). The body is sent and dropped; a failure is thrown as `ServerError` with the server's words. */
  async post(path: string, body: Record<string, unknown>): Promise<void> {
    let res: Response;
    try { res = await this.f(`${this.base}${path}`, { method: "POST", headers: this.headers({ "content-type": "application/json" }), body: JSON.stringify(body) }); }
    catch (e) { throw new ServerError("unavailable", (e as Error).message); }
    if (res.ok) return;
    const j = (await res.json().catch(() => null)) as { error?: { code?: ApiError["code"]; message?: string } } | null;
    throw new ServerError(j?.error?.code ?? "internal", j?.error?.message ?? `refused (${res.status})`, res.status);
  }
  spaces = { list: () => this.operate<SpacesListResult>("spaces.list") };
  notes = {
    list: (space_id: string, tag?: string, cursor?: string) => this.operate<NotesListResult>("notes.list", { space_id, ...(tag ? { tag } : {}), ...(cursor ? { cursor } : {}) }),
    /** `space_id` is only a hint (the server finds the owning space from the id): empty omits it, which is what a note shared from another person's space needs. */
    get: (space_id: string, id: string) => this.operate<NoteResult>("notes.get", { ...(space_id ? { space_id } : {}), id }),
    create: (i: NotesCreateInput, requestId?: string) => this.operate<NoteResult>("notes.create", i as unknown as Record<string, unknown>, requestId),
    update: (i: NotesUpdateInput, requestId?: string) => this.operate<NoteResult>("notes.update", i as unknown as Record<string, unknown>, requestId),
    history: (space_id: string, id: string) => this.operate<NotesHistoryResult>("notes.history", { space_id, id }),
  };
  /** Private drafts: a complete replacement per (space, item); never shared with other members. */
  drafts = {
    save: (space_id: string, item_id: string, text: string) => this.operate<AccountInputResult>("drafts.save", { space_id, item_id, text }),
    list: () => this.operate<AccountInputListResult>("drafts.list"),
    get: (space_id: string, item_id: string) => this.operate<AccountInputResult>("drafts.get", { space_id, item_id }),
    delete: (space_id: string, item_id: string) => this.operate<AccountInputResult>("drafts.delete", { space_id, item_id }),
  };
  /** Pending submissions: a write queued with its original creation time; the nested request's id is the record's id. */
  pending = {
    queue: (space_id: string, item_id: string, created_at_ms: number, request: { request_id: string; operation: string; input: Record<string, unknown> }) => this.operate<AccountInputResult>("pending.queue", { space_id, item_id, created_at_ms, request }),
    edit: (space_id: string, item_id: string, created_at_ms: number, request: { request_id: string; operation: string; input: Record<string, unknown> }) => this.operate<AccountInputResult>("pending.edit", { space_id, item_id, created_at_ms, request }),
    list: () => this.operate<AccountInputListResult>("pending.list"),
    cancel: (id: string) => this.operate<AccountInputResult>("pending.cancel", { id }),
    confirm: (id: string) => this.operate<AccountInputResult>("pending.confirm", { id }),
    submit: (id: string) => this.operate<AccountInputResult>("pending.submit", { id }),
  };
  jobs = {
    get: (job_id: string) => this.operate<unknown>("jobs.get", { job_id }),
    cancel: (job_id: string) => this.operate<unknown>("jobs.cancel", { job_id }),
  };

  /**
   * Follows the live space stream (`GET /api/v1/live`, SSE) for the given subscriptions, calling `onFrame` for each frame (an entry, ready with a bookmark, or a reset).
   * Resolves when the stream ends or `signal` aborts. A reset means: reload the derived state, then subscribe again.
   */
  async live(subs: LiveSubscription[], onFrame: (f: LiveFrame) => void, signal?: AbortSignal): Promise<void> {
    for await (const ev of events(await this.stream(`/api/v1/live?subscriptions=${encodeURIComponent(JSON.stringify(subs))}`, signal))) {
      const raw = JSON.parse(ev.data) as Record<string, unknown>;
      // The wire form is internally tagged (`{"type":"entry",...}`); the generated type names the variants externally. Accept both.
      const type = typeof raw.type === "string" ? raw.type : Object.keys(raw)[0]?.toLowerCase();
      const body = typeof raw.type === "string" ? raw : (Object.values(raw)[0] as Record<string, unknown>);
      onFrame(({ [type === "entry" ? "Entry" : type === "ready" ? "Ready" : "Reset"]: body } as unknown) as LiveFrame);
    }
  }

  /**
   * Follows the signed-in account's feed (`GET /api/v1/account/live`, SSE) and hands each frame to `onFrame` as the contract's `AccountFrame`: `Changed` says the private snapshot (drafts, pending) is stale (reload
   * `drafts.list` and `pending.list`, then keep the bookmark), `Ready` carries the bookmark, `Reset` says to reload and subscribe again without one, and `ReadinessChanged` (only when `readiness` is asked for) says to read
   * `agent.readiness` again. This is the one place that reads the wire form of the feed (internally tagged, `{"type":"changed",…}`, against the generated externally tagged type); every reader gets the same frames.
   */
  async accountLive(onFrame: (f: AccountFrame) => void, o: { bookmark?: string; readiness?: boolean; signal?: AbortSignal } = {}): Promise<void> {
    const q = [o.bookmark ? `bookmark=${encodeURIComponent(o.bookmark)}` : "", o.readiness ? "readiness=true" : ""].filter(Boolean).join("&");
    for await (const ev of events(await this.stream(`/api/v1/account/live${q ? `?${q}` : ""}`, o.signal))) {
      const raw = JSON.parse(ev.data) as Record<string, unknown> | string;
      const type = (typeof raw === "string" ? raw : typeof raw.type === "string" ? raw.type : Object.keys(raw)[0] ?? "").toLowerCase().replace(/_/g, "");
      const body = typeof raw === "object" && typeof raw.type !== "string" ? (Object.values(raw)[0] as Record<string, unknown>) : (raw as Record<string, unknown>);
      if (type === "readinesschanged") onFrame("ReadinessChanged");
      else if (type === "reset") onFrame("Reset");
      else onFrame(({ [type === "ready" ? "Ready" : "Changed"]: { bookmark: String((typeof body === "object" ? body?.bookmark : "") ?? "") } }) as AccountFrame);
    }
  }
  /** The readiness feed: calls `onChange` whenever `agent.readiness` should be read again (readiness changed, or a reset). Resolves when the stream ends or `signal` aborts. */
  readinessLive = (onChange: () => void, signal?: AbortSignal): Promise<void> => this.accountLive((f) => { if (f === "ReadinessChanged" || f === "Reset") onChange(); }, { readiness: true, signal });

  /** Follows a plugin job's events (`GET /api/v1/jobs/<id>/events`, SSE): `snapshot` first, then `chunk`s, possibly a `reset`, and `end`. `after` resumes (`<epoch>:<seq>`). */
  async jobEvents(jobId: string, onEvent: (name: "snapshot" | "chunk" | "reset" | "state" | "end", data: unknown, id?: string) => void, o: { after?: string; signal?: AbortSignal } = {}): Promise<void> {
    const q = o.after ? `?after=${encodeURIComponent(o.after)}` : "";
    for await (const ev of events(await this.stream(`/api/v1/jobs/${encodeURIComponent(jobId)}/events${q}`, o.signal))) onEvent(ev.event as "snapshot", ev.data ? JSON.parse(ev.data) : null, ev.id);
  }
}

const KEY = "cal.devtoken";
/** The development token: from `?token=` (stored, then removed from the address), else from localStorage. Null when there is none. */
export function devToken(): string | null {
  try {
    const q = new URLSearchParams(location.search).get("token");
    if (q) {
      localStorage.setItem(KEY, q);
      const u = new URL(location.href);
      u.searchParams.delete("token");
      history.replaceState(null, "", u.pathname + (u.search ? u.search : "") + u.hash);
    }
    return localStorage.getItem(KEY);
  } catch { return null; }
}
/**
 * Whether the real server is the source: `?server=1` or `?server=0` (kept in `cal.server`), else the build's default: fixtures, except a build made with `VITE_SERVER_DEFAULT=1` (the preview host's, scripts/deploy-preview.sh),
 * which talks to its server unless `?server=0` is given.
 */
export function serverMode(): boolean {
  try {
    // An invitation link is for the real server, whoever opens it (the person has no flag set).
    if (/\/invites\/[^/?#]+/.test(location.pathname)) localStorage.setItem("cal.server", "1");
    const q = new URLSearchParams(location.search).get("server");
    if (q === "1" || q === "0") localStorage.setItem("cal.server", q);
    const kept = localStorage.getItem("cal.server");
    return kept === null ? import.meta.env.VITE_SERVER_DEFAULT === "1" : kept === "1";
  } catch { return import.meta.env.VITE_SERVER_DEFAULT === "1"; }
}
/** The shared client (development token mode). */
export const server = new ServerClient({ token: devToken });
// Read the token at load, before the router rewrites the address: `?token=` is stored and removed at once.
void devToken();
