/**
 * The real server's client, for what the server on origin/dev already offers: `describe`, spaces, notes, plugins, the live space stream (SSE) and the
 * jobs stream (SSE). Types come from `packages/contracts` (generated from the Rust contract). Same origin: in development the Vite server proxies `/api` to
 * the server on 127.0.0.1 (vite.config.ts, `CAL_SERVER`), which is also how its Host and Origin checks are satisfied.
 *
 * Sign-in is not wired for browsers yet (passkeys exist on the server; the web flow does not). Until it is, this uses the DEVELOPMENT TOKEN MODE: a bearer token
 * from `calternal serve --development` (written to `<data>/dev-tokens/<principal>.token`), given once as `?token=` and kept in localStorage; it is never printed.
 * Every call is one `POST /api/v1/operations` with a fresh `request_id` (kept on retry), and a failure is thrown as `ServerError` with the server's safe code.
 */
import type {
  AccountFrame, AccountInputListResult, AccountInputResult, ApiError, DescribeResult, LiveFrame, LiveSubscription, NoteResult, NotesCreateInput, NotesHistoryResult, NotesListResult, NotesUpdateInput, OperationResponse,
  SpacesListResult,
} from "../../../../packages/contracts/src/index";
import { events } from "./sse";
import { link } from "./link.svelte";

/** A failure the server reported (`code` is its stable category) or a transport failure (`code` is "unavailable"). */
export class ServerError extends Error {
  constructor(public code: ApiError["code"], message: string, public status = 0) { super(message); }
}

export interface ServerOptions { base?: string; token: () => string | null; fetch?: typeof fetch }

/** The server's client. */
export class ServerClient {
  private base: string;
  private f: typeof fetch;
  private token: () => string | null;
  constructor(o: ServerOptions) {
    this.base = o.base ?? "";
    this.token = o.token;
    const raw = o.fetch ?? ((...a: Parameters<typeof fetch>) => fetch(...a));
    // Every call reports to the shared link state (reconnecting banner, expired session); a call the caller cancelled is not a failure.
    this.f = (...a) => raw(...a).then((r) => { if (r.status === 401) link.expired(!!this.token()); else link.ok(); return r; }, (e: Error) => { if (e?.name !== "AbortError") link.failed(); throw e; });
  }
  /** The headers a plain `fetch` of an API path needs (the bearer in development token mode; nothing for a cookie session). */
  authHeaders(): Record<string, string> { return this.headers(); }
  /** A plain call to an API path (the machines routes) with the session, reporting to the link state like every other call. */
  request(path: string, init: RequestInit = {}): Promise<Response> { return this.f(`${this.base}${path}`, { ...init, headers: this.headers(init.headers as Record<string, string>) }); }
  private headers(extra: Record<string, string> = {}): Record<string, string> {
    const t = this.token();
    return { ...(t ? { authorization: `Bearer ${t}` } : {}), ...extra };
  }
  /** Calls one operation. `requestId` makes a retry the same request. */
  async operate<T>(operation: string, input: Record<string, unknown> = {}, requestId: string = crypto.randomUUID()): Promise<T> {
    let res: Response;
    try {
      res = await this.f(`${this.base}/api/v1/operations`, { method: "POST", headers: this.headers({ "content-type": "application/json" }), body: JSON.stringify({ request_id: requestId, operation, input }) });
    } catch (e) {
      throw new ServerError("unavailable", "Can't reach the server.");
    }
    let body: OperationResponse<T>;
    try { body = (await res.json()) as OperationResponse<T>; } catch { throw new ServerError("internal", `unreadable answer (${res.status})`, res.status); }
    if (!body.ok) throw new ServerError(body.error.code, body.error.message, res.status);
    return body.data;
  }
  /** What the server offers and who is asking (`GET /api/v1/describe`). */
  async describe(): Promise<DescribeResult> {
    const res = await this.f(`${this.base}/api/v1/describe`, { headers: this.headers() }).catch((e: Error) => { throw new ServerError("unavailable", e.message); });
    // Unlike an operation, discovery answers with the description itself (no envelope); a failure still arrives as a failure envelope.
    const body = (await res.json()) as DescribeResult | OperationResponse<never>;
    if ("ok" in body && body.ok === false) throw new ServerError(body.error.code, body.error.message, res.status);
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
    list: (space_id: string) => this.operate<NotesListResult>("notes.list", { space_id }),
    get: (space_id: string, id: string) => this.operate<NoteResult>("notes.get", { space_id, id }),
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
    const res = await this.f(`${this.base}/api/v1/live?subscriptions=${encodeURIComponent(JSON.stringify(subs))}`, { headers: this.headers({ accept: "text/event-stream" }), signal }).catch((e: Error) => { throw new ServerError("unavailable", e.message); });
    if (!res.ok || !res.body) throw new ServerError("unavailable", `live stream refused (${res.status})`, res.status);
    for await (const ev of events(res.body)) {
      const raw = JSON.parse(ev.data) as Record<string, unknown>;
      // The wire form is internally tagged (`{"type":"entry",...}`); the generated type names the variants externally. Accept both.
      const type = typeof raw.type === "string" ? raw.type : Object.keys(raw)[0]?.toLowerCase();
      const body = typeof raw.type === "string" ? raw : (Object.values(raw)[0] as Record<string, unknown>);
      onFrame(({ [type === "entry" ? "Entry" : type === "ready" ? "Ready" : "Reset"]: body } as unknown) as LiveFrame);
    }
  }

  /**
   * Follows the signed-in account's feed (`GET /api/v1/account/live`, SSE): `Changed` says the private snapshot (drafts, pending) is stale (reload `drafts.list` and
   * `pending.list`, then keep the bookmark), `Ready` carries the bookmark, `Reset` says to reload and subscribe again without one.
   */
  async accountLive(onFrame: (f: AccountFrame) => void, o: { bookmark?: string; signal?: AbortSignal } = {}): Promise<void> {
    const q = o.bookmark ? `?bookmark=${encodeURIComponent(o.bookmark)}` : "";
    const res = await this.f(`${this.base}/api/v1/account/live${q}`, { headers: this.headers({ accept: "text/event-stream" }), signal: o.signal }).catch((e: Error) => { throw new ServerError("unavailable", e.message); });
    if (!res.ok || !res.body) throw new ServerError("unavailable", `account stream refused (${res.status})`, res.status);
    for await (const ev of events(res.body)) {
      const raw = JSON.parse(ev.data) as Record<string, unknown> | string;
      // Internally tagged on the wire (`{"type":"changed","bookmark":...}`), externally in the generated type: accept both.
      const type = typeof raw === "string" ? raw : typeof raw.type === "string" ? raw.type : Object.keys(raw)[0]?.toLowerCase();
      const body = typeof raw === "object" && typeof raw.type !== "string" ? (Object.values(raw)[0] as Record<string, unknown>) : (raw as Record<string, unknown>);
      onFrame(type === "reset" ? "Reset" : ({ [type === "ready" ? "Ready" : "Changed"]: { bookmark: String(body.bookmark ?? "") } } as unknown as AccountFrame));
    }
  }

  /** Follows a plugin job's events (`GET /api/v1/jobs/<id>/events`, SSE): `snapshot` first, then `chunk`s, possibly a `reset`, and `end`. `after` resumes (`<epoch>:<seq>`). */
  async jobEvents(jobId: string, onEvent: (name: "snapshot" | "chunk" | "reset" | "state" | "end", data: unknown, id?: string) => void, o: { after?: string; signal?: AbortSignal } = {}): Promise<void> {
    const q = o.after ? `?after=${encodeURIComponent(o.after)}` : "";
    const res = await this.f(`${this.base}/api/v1/jobs/${encodeURIComponent(jobId)}/events${q}`, { headers: this.headers({ accept: "text/event-stream" }), signal: o.signal }).catch((e: Error) => { throw new ServerError("unavailable", e.message); });
    if (!res.ok || !res.body) throw new ServerError(res.status === 404 ? "not_found" : "unavailable", `job stream refused (${res.status})`, res.status);
    for await (const ev of events(res.body)) onEvent(ev.event as "snapshot", ev.data ? JSON.parse(ev.data) : null, ev.id);
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
/** Whether the real server is the source: `?server=1` (kept) or `cal.server` in localStorage. Fixtures are the default. */
export function serverMode(): boolean {
  try {
    const q = new URLSearchParams(location.search).get("server");
    if (q === "1" || q === "0") localStorage.setItem("cal.server", q);
    return localStorage.getItem("cal.server") === "1";
  } catch { return false; }
}
/** The shared client (development token mode). */
export const server = new ServerClient({ token: devToken });
// Read the token at load, before the router rewrites the address: `?token=` is stored and removed at once.
void devToken();
