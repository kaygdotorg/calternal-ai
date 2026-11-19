/**
 * The wire. A `Transport` carries the catalogue's sealed events to the host and returns its sealed replies; it knows nothing of what they mean. `HttpTransport` speaks the step 0 host (a bearer, one POST route per
 * event, the view lease in `X-Step0-View-Lease`); the routes are the fixture's, the bodies are the catalogue's. A page that relays for the host, or the real host, supplies another transport and the rest is unchanged.
 */

/** One route of the exchange; the names are the catalogue's event families. */
export type Route =
  | "view.open" | "view.renew" | "window" | "selection.settled" | "selection.set"
  | "activation.resolve" | "activation.confirm" | "activation.dispatch" | "choice.options" | "undo" | "status";

/** A host answer that is not a sealed reply: the HTTP status and, when the body said so, the host's own code and message. */
export class HostError extends Error {
  constructor(readonly status: number, readonly code: string | undefined, message: string, readonly body?: unknown) { super(message); this.name = "HostError"; }
}

/** Sends one event and resolves with the host's JSON reply. A 422 window refusal is a reply, not an error. */
export interface Transport {
  post<T>(route: Route, body: unknown, opts?: { lease?: string; signal?: AbortSignal }): Promise<T>;
}

/** Options of the HTTP transport. */
export interface HttpTransportOptions {
  /** Base address, for example `http://127.0.0.1:43125/step0/v2`. */
  base: string;
  /** The disposable bearer the launcher printed. Held in memory only; never logged. */
  bearer?: string;
  /** Replaceable for tests. */
  fetch?: typeof fetch;
}

/** The step 0 host over HTTP. */
export class HttpTransport implements Transport {
  readonly #base: string; readonly #bearer?: string; readonly #fetch: typeof fetch;
  constructor(o: HttpTransportOptions) { this.#base = o.base.replace(/\/$/, ""); this.#bearer = o.bearer; this.#fetch = o.fetch ?? ((...a) => fetch(...a)); }

  async post<T>(route: Route, body: unknown, opts: { lease?: string; signal?: AbortSignal } = {}): Promise<T> {
    const headers: Record<string, string> = { "content-type": "application/json" };
    if (this.#bearer) headers.authorization = `Bearer ${this.#bearer}`;
    if (opts.lease) headers["x-step0-view-lease"] = opts.lease;
    const res = await this.#fetch(`${this.#base}/${route}`, { method: "POST", headers, body: JSON.stringify(body), signal: opts.signal });
    const text = await res.text();
    let json: unknown; try { json = text ? JSON.parse(text) : undefined; } catch { json = undefined; }
    // The catalogue's typed window refusal (status 422, code unsupported_query_plan) is part of `WindowReply`: hand it back whole.
    if (res.ok || (res.status === 422 && (json as { code?: string } | undefined)?.code === "unsupported_query_plan")) return json as T;
    const j = json as { code?: string; message?: string } | undefined;
    throw new HostError(res.status, j?.code, j?.message ?? `The host answered ${res.status}.`, json);
  }
}
