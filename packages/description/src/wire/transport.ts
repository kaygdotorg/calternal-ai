/**
 * The wire. A `Transport` carries the catalogue's sealed events to the host and returns its sealed replies; it knows nothing of what they mean. `HttpTransport` speaks the step 0 host (a bearer, one POST route per
 * event, the view lease in `X-Step0-View-Lease`); the routes are the fixture's, the bodies are the catalogue's. A page that relays for the host, or the real host, supplies another transport and the rest is unchanged.
 */

/** One route of the exchange; the names are the catalogue's event families. */
export type Route =
  | "view.open" | "view.renew" | "window" | "selection.settled" | "selection.set"
  | "search" | "activation.resolve" | "activation.confirm" | "activation.dispatch" | "choice.options" | "undo" | "status";

/** A host answer that is not a sealed reply: the HTTP status and, when the body said so, the host's own code and message. */
export class HostError extends Error {
  constructor(readonly status: number, readonly code: string | undefined, message: string, readonly body?: unknown) { super(message); this.name = "HostError"; }
}

/** Sends one event and resolves with the host's JSON reply. A 422 window refusal is a reply, not an error. */
export interface Transport {
  /** Reads a host-authorized row image (`GET row-images/{ref}`); null for 404 (absent, expired, stale or unauthorized). Optional: a transport without it draws the fallback miniature. */
  image?(ref: string, opts?: { lease?: string }): Promise<Blob | null>;
  post<T>(route: Route, body: unknown, opts?: { lease?: string; signal?: AbortSignal; route?: Record<string, string> }): Promise<T>;
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

  async image(ref: string, opts: { lease?: string } = {}): Promise<Blob | null> {
    const headers: Record<string, string> = {};
    if (this.#bearer) headers.authorization = `Bearer ${this.#bearer}`;
    if (opts.lease) headers["x-step0-view-lease"] = opts.lease;
    const res = await this.#fetch(`${this.#base}/row-images/${encodeURIComponent(ref)}`, { headers, redirect: "error", cache: "no-store" });
    if (!res.ok) return null;
    const type = res.headers.get("content-type") ?? "";
    if (!/^image\/(png|jpeg)/.test(type)) return null;
    const blob = await res.blob();
    return blob.size <= 65536 ? blob : null; // the catalogue's byte cap
  }

  async post<T>(route: Route, body: unknown, opts: { lease?: string; signal?: AbortSignal; route?: Record<string, string> } = {}): Promise<T> {
    const headers: Record<string, string> = { "content-type": "application/json" };
    if (this.#bearer) headers.authorization = `Bearer ${this.#bearer}`;
    if (opts.lease) headers["x-step0-view-lease"] = opts.lease;
    // The step 0 fixture takes the route parameters as headers (X-Step0-Route-Item): the sealed contract has the host read them from the view, so this stays in the fixture transport.
    for (const [k, v] of Object.entries(opts.route ?? {})) headers[`x-step0-route-${k}`] = v;
    const res = await this.#fetch(`${this.#base}/${route}`, { method: "POST", headers, body: JSON.stringify(body), signal: opts.signal });
    const text = await res.text();
    let json: unknown; try { json = text ? JSON.parse(text) : undefined; } catch { json = undefined; }
    // The catalogue's typed window refusal (status 422, code unsupported_query_plan) is part of `WindowReply`: hand it back whole.
    if (res.ok || (res.status === 422 && (json as { code?: string } | undefined)?.code === "unsupported_query_plan")) return json as T;
    const j = json as { code?: string; message?: string } | undefined;
    throw new HostError(res.status, j?.code, j?.message ?? `The host answered ${res.status}.`, json);
  }
}

/** Options of the product transport. */
export interface ProductTransportOptions {
  /** The view's address, for example `http://127.0.0.1:3210/api/v2/spaces/<space>/reading-list-view` (the same transport serves a Notes view at its own address). */
  base: string;
  /** The signed-in session's bearer token; held in memory only. */
  bearer: string;
  fetch?: typeof fetch;
}

/**
 * The product routes of a host view (docs/reviews/window-engine-2026-11-19.md): `GET base` describes the view, `POST base` opens it, `POST base/window` and `POST base/renew` run under the lease, which travels as
 * `X-Calternal-View-Lease` with its cache partition in `X-Calternal-Cache-Partition`. Window refusals are the sealed 422 body. Routes the product slice does not serve (activation, search, selection) fail with a
 * 501 `HostError`, so the renderer reports them as unavailable rather than inventing an answer.
 */
export class ProductTransport implements Transport {
  readonly #base: string; readonly #bearer: string; readonly #fetch: typeof fetch;
  #partition = "";
  constructor(o: ProductTransportOptions) { this.#base = o.base.replace(/\/$/, ""); this.#bearer = o.bearer; this.#fetch = o.fetch ?? ((...a) => fetch(...a)); }

  /** The view's own description: its `view_ref` and the emitted description. */
  async describe<T = { view_ref: string; description: unknown }>(): Promise<T> {
    const res = await this.#fetch(this.#base, { headers: { authorization: `Bearer ${this.#bearer}` } });
    if (!res.ok) throw new HostError(res.status, undefined, `The host answered ${res.status}.`);
    return (await res.json()) as T;
  }

  async post<T>(route: Route, body: unknown, opts: { lease?: string; signal?: AbortSignal } = {}): Promise<T> {
    const path = route === "view.open" ? "" : route === "window" ? "/window" : route === "view.renew" ? "/renew" : null;
    if (path === null) throw new HostError(501, "unsupported_route", `This product route does not serve ${route}.`);
    const headers: Record<string, string> = { "content-type": "application/json", authorization: `Bearer ${this.#bearer}` };
    if (opts.lease) { headers["x-calternal-view-lease"] = opts.lease; headers["x-calternal-cache-partition"] = this.#partition; }
    const res = await this.#fetch(`${this.#base}${path}`, { method: "POST", headers, body: JSON.stringify(body), signal: opts.signal });
    const text = await res.text();
    let json: unknown; try { json = text ? JSON.parse(text) : undefined; } catch { json = undefined; }
    if (res.ok || (res.status === 422 && (json as { code?: string } | undefined)?.code === "unsupported_query_plan")) {
      const lease = (json as { lease?: { cache_partition?: string } } | undefined)?.lease;
      if (lease?.cache_partition) this.#partition = lease.cache_partition;
      return json as T;
    }
    const j = json as { code?: string; message?: string } | undefined;
    throw new HostError(res.status, j?.code, j?.message ?? `The host answered ${res.status}.`, json);
  }
}
