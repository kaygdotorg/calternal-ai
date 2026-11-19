/**
 * One open view against the host: view open, the shared subscription lease and its renewal, and the window route with the lease attached. The lease is 15 seconds (`SubscriptionLease.lease_seconds`);
 * the session renews it while rows are painted and reports `current` false while a renewal is outstanding past the lease, so the interface keeps filled rows in memory but does not paint them as current
 * (docs/description-contract.md section 6). Activation contexts and refs are held in memory only.
 */
import type { ActionContexts, SelectionContextResult, SettledSelectionRequest, SubscriptionLease, ViewOpenResult, WindowAudience, WindowReply, WindowRequestPayload, LeaseRenewRequest, LeaseRenewResult } from "../model";
import type { Route, Transport } from "./transport";

/** What a session needs from outside: the transport, a clock and a timer (replaceable for tests). */
export interface SessionOptions {
  transport: Transport;
  /** Where renewal asks which window refs and result versions the interface holds. */
  held?: () => Record<string, string>;
}

/** An open view: the lease, the audience and the contexts for the view's own (header, form, composer) actions. */
export class HostSession {
  readonly transport: Transport;
  #lease: SubscriptionLease | null = null;
  #audience: WindowAudience | null = null;
  #viewContexts: ActionContexts = {};
  /** The route parameters now in force (selector, open item), for hosts that read them from the transport. */
  route: Record<string, string> = {};
  #renewalSeq = 0;
  #closed = false;
  #lapsed = false;
  #lapseTimer: ReturnType<typeof setTimeout> | null = null;
  #timer: ReturnType<typeof setTimeout> | null = null;
  #held: () => Record<string, string>;
  /** What the interface holds, for renewal: window ref to result version. */
  set held(f: () => Record<string, string>) { this.#held = f; }
  /** Called whenever the view's own activation contexts were replaced (at open and at each renewal), so what is drawn from them can update. */
  oncontexts?: () => void;
  /** Called with false when the lease lapses without a renewal (rows stay in memory, hidden) and with true when a renewal grants it again (which windows repaint is `onrenewed`'s to decide). */
  oncurrent?: (current: boolean) => void;
  /** Called with replacement windows a renewal returned. */
  onrenewed?: (r: LeaseRenewResult) => void;

  constructor(o: SessionOptions) { this.transport = o.transport; this.#held = o.held ?? (() => ({})); }

  get lease(): SubscriptionLease | null { return this.#lease; }
  get audience(): WindowAudience | null { return this.#audience; }
  /** Contexts issued with the view for actions that belong to no row. */
  get viewContexts(): ActionContexts { return this.#viewContexts; }
  /** True while the lease has not lapsed: a lapse timer fires `lease_seconds` after the last grant or renewal, however the renewals went (failed, hung or ignored). */
  get current(): boolean { return !!this.#lease && !this.#lapsed; }

  /** Opens `viewRef` (a ref the host issued: no id is made up here) and starts the renewal timer. */
  async open(viewRef: string): Promise<ViewOpenResult> {
    this.#closed = false;
    const r = await this.transport.post<ViewOpenResult>("view.open", { type: "view_open", view_ref: viewRef });
    this.#take(r.lease); this.#audience = r.audience; this.#viewContexts = r.activation_contexts; this.oncontexts?.();
    this.#schedule();
    return r;
  }

  /** One window request under the lease. Resolves with the host's reply (a typed refusal included). */
  window(request: WindowRequestPayload, signal?: AbortSignal): Promise<WindowReply> {
    if (!this.#lease) return Promise.reject(new Error("The view is not open."));
    return this.transport.post<WindowReply>("window", { type: "window", request }, { lease: this.#lease.lease_ref, signal, route: this.route });
  }

  /** Tells the host which single row is now selected (the settled selection, section 3.2) and returns the contexts it issued for exactly that row. */
  selection(bind: string, row: string, signal?: AbortSignal): Promise<SelectionContextResult> {
    if (!this.#lease) return Promise.reject(new Error("The view is not open."));
    const body: SettledSelectionRequest = { type: "selection", bind, row };
    return this.transport.post<SelectionContextResult>("selection.settled", body, { lease: this.#lease.lease_ref, signal });
  }

  /** A row image by its host-issued ref, under the lease; null when none (the row draws its fallback). `signal` cancels the read (the row left the screen). */
  async image(ref: string, signal?: AbortSignal): Promise<Blob | null> {
    if (!this.#lease || !this.transport.image) return null;
    try { return await this.transport.image(ref, { lease: this.#lease.lease_ref, signal }); } catch { return null; }
  }

  /** Any other sealed event under the lease (activation, choice options, status, undo, selection set). */
  exchange<T>(route: Route, body: unknown, signal?: AbortSignal): Promise<T> {
    if (!this.#lease) return Promise.reject(new Error("The view is not open."));
    return this.transport.post<T>(route, body, { lease: this.#lease.lease_ref, signal });
  }

  #take(l: SubscriptionLease) {
    this.#lease = l; this.#lapsed = false;
    if (this.#lapseTimer) clearTimeout(this.#lapseTimer);
    // The lease runs out `lease_seconds` after it was granted unless a renewal grants it again: rows are then not current until a renewal names them (section 6).
    this.#lapseTimer = setTimeout(() => { if (this.#closed) return; this.#lapsed = true; this.oncurrent?.(false); }, l.lease_seconds * 1000);
  }

  #schedule() {
    if (this.#timer) clearTimeout(this.#timer);
    const l = this.#lease; if (!l) return;
    // Renew at two thirds of the lease so one lost attempt still leaves time for a second.
    this.#timer = setTimeout(() => { void this.renew(); }, l.lease_seconds * 1000 * 0.66);
  }

  /** Renews the lease. A superseded or refused answer keeps the rows hidden; a matching one repaints only what it retained. */
  async renew(): Promise<void> {
    const l = this.#lease; if (!l) return;
    const renewal_id = `r-${++this.#renewalSeq}-${Math.random().toString(36).slice(2, 8)}`;
    const req: LeaseRenewRequest = { type: "lease_renew", renewal_id, lease_ref: l.lease_ref, cache_partition: l.cache_partition, grant_revision: l.grant_revision, result_versions: this.#held() };
    const sent = this.#renewalSeq;
    try {
      const r = await this.transport.post<LeaseRenewResult>("view.renew", req, { lease: l.lease_ref });
      if (this.#closed || this.#lease !== l) return; // the view closed (or its lease moved on) while the renewal was in flight: nothing to take, no timer to start
      // A reply that is not the answer to this renewal (superseded, another lease or partition) is ignored; the next attempt is still scheduled.
      if (!(sent !== this.#renewalSeq || r.renewal_id !== renewal_id || r.request_lease_ref !== l.lease_ref || r.cache_partition !== l.cache_partition)) {
        this.#take(r.lease); this.#viewContexts = r.activation_contexts; this.oncontexts?.(); this.oncurrent?.(true); this.onrenewed?.(r);
      }
    } catch { /* the lapse timer hides the rows when the lease runs out */ }
    if (this.#closed) return;
    this.#schedule();
  }

  /** Stops renewing (the view closed). */
  close() { this.#closed = true; if (this.#lapseTimer) clearTimeout(this.#lapseTimer); if (this.#timer) clearTimeout(this.#timer); this.#timer = null; this.#lease = null; }
}
