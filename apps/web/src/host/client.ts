/**
 * HTTP call route client, passkey auth + ADR 0030 logout lifecycle, and
 * single-WebSocket live window manager for the real `calternal-host`.
 *
 * Speaks `POST /v1/spaces/{space}/call` (docs/reviews/host-call-route-2026-11-27.md)
 * and `GET /v1/spaces/{space}/live` (docs/reviews/live-window-protocol-2026-11-27.md).
 */
import type {
  HostError,
  HostSessionStatus,
  ItemDetail,
  LiveItemMetadata,
  LiveServerFrame,
  QuerySpec,
  RecordData,
  RowData,
  SessionInfo,
  SpaceDeclarations,
} from "./types";

const SESSION_KEY = "calternal.host.session-info";
const PENDING_LOGOUT_COOKIE = "calternal_pending_logout";
const AUTH_CHANNEL_NAME = "calternal-auth";
const REFUSED_AT_ONCE_MS = 400;

/** Read a saved session display summary from `sessionStorage` (isolated per tab/context). */
export function loadSession(): SessionInfo | null {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY);
    return raw ? (JSON.parse(raw) as SessionInfo) : null;
  } catch {
    return null;
  }
}

/** Save or clear the active session display summary in `sessionStorage`. */
export function saveSession(session: SessionInfo | null): void {
  try {
    if (session) sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
    else sessionStorage.removeItem(SESSION_KEY);
  } catch {
    // ignore storage errors in restricted environments
  }
}

/** Delete all account-scoped browser storage (ADR 0030). */
export function clearAccountStorage(): void {
  saveSession(null);
  try {
    const toRemove: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k && k.startsWith("calternal.host.")) toRemove.push(k);
    }
    for (const k of toRemove) localStorage.removeItem(k);
  } catch {
    // ignore storage errors in restricted environments
  }
}

/** Whether a previous offline sign-out is still waiting to revoke its installation (ADR 0030). */
export function hasPendingLogout(): boolean {
  try {
    return document.cookie.split(";").some((c) => c.trim().startsWith(`${PENDING_LOGOUT_COOKIE}=1`));
  } catch {
    return false;
  }
}

function setPendingLogout(pending: boolean): void {
  try {
    if (pending) {
      document.cookie = `${PENDING_LOGOUT_COOKIE}=1; path=/; SameSite=Strict`;
    } else {
      document.cookie = `${PENDING_LOGOUT_COOKIE}=; path=/; max-age=0; SameSite=Strict`;
    }
  } catch {
    // ignore cookie errors
  }
}

/** Notify other open tabs on the same origin that this browser signed out (ADR 0030). */
export function broadcastSignOut(): void {
  try {
    if (typeof BroadcastChannel !== "undefined") {
      const ch = new BroadcastChannel(AUTH_CHANNEL_NAME);
      ch.postMessage({ type: "logout" });
      ch.close();
    }
  } catch {
    // ignore BroadcastChannel errors
  }
}

/** Listen for sign-out broadcasts from other tabs on the same origin (ADR 0030). */
export function subscribeAuthChannel(onLogout: () => void): () => void {
  if (typeof BroadcastChannel === "undefined") return () => {};
  const ch = new BroadcastChannel(AUTH_CHANNEL_NAME);
  const handler = (ev: MessageEvent) => {
    if (ev.data && typeof ev.data === "object" && (ev.data as { type?: string }).type === "logout") {
      clearAccountStorage();
      onLogout();
    }
  };
  ch.addEventListener("message", handler);
  return () => {
    ch.removeEventListener("message", handler);
    ch.close();
  };
}

/** Typed caller-safe error thrown when a host route returns an error payload. */
export class HostCallError extends Error {
  readonly code: string;
  readonly next: string;
  constructor(err: HostError) {
    super(err.message);
    this.code = err.code;
    this.next = err.next;
  }
}

interface Ceremony {
  ceremony: string;
  options: { publicKey: unknown };
}

/** Result returned by `setupHost`, including one-time recovery codes to display once. */
export interface SessionReply {
  session: SessionInfo;
  recovery_codes?: string[];
}

/** Slot filler selection passed at the envelope level of `POST /v1/spaces/{space}/call`. */
export interface SlotSelection {
  slot: string;
  filler: string;
}

async function hostJson<T>(path: string, body?: Record<string, unknown>): Promise<T> {
  const res = await fetch(
    path,
    body === undefined
      ? {}
      : {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body),
        },
  );
  const data = res.status === 204 ? undefined : await res.json().catch(() => undefined);
  if (!res.ok) {
    throw new HostCallError(
      data?.error ?? {
        code: "unreachable",
        message: "The host could not complete the call.",
        next: "Try again.",
      },
    );
  }
  return data as T;
}

/** Retry any pending offline logout before checking session status (ADR 0030). */
export async function retryPendingLogout(): Promise<boolean> {
  if (!hasPendingLogout()) return true;
  try {
    await hostJson<void>("/v1/auth/logout", {});
    setPendingLogout(false);
    clearAccountStorage();
    return true;
  } catch {
    return false;
  }
}

/** Check the cookie-backed session and whether first-owner enrollment is available. */
export async function sessionStatus(): Promise<HostSessionStatus> {
  if (hasPendingLogout()) {
    const cleared = await retryPendingLogout();
    if (!cleared) {
      saveSession(null);
      return { setup: false, session: null };
    }
  }
  const status = await hostJson<HostSessionStatus>("/v1/auth/status");
  saveSession(status.session);
  return status;
}

function mapPasskeyError(err: unknown, elapsedMs: number): HostCallError {
  if (err instanceof HostCallError) return err;
  const name = err instanceof Error ? err.name : "";
  if (name === "InvalidStateError") {
    return new HostCallError({
      code: "conflict",
      message: "This device already has a passkey here. Sign in instead.",
      next: "Use Sign in with a passkey.",
    });
  }
  if (name === "SecurityError") {
    return new HostCallError({
      code: "not-allowed",
      message: "This page's address cannot be used for passkeys.",
      next: "Open the configured origin.",
    });
  }
  if (name === "NotSupportedError") {
    return new HostCallError({
      code: "invalid",
      message: "This device or browser does not support passkeys.",
      next: "Use a browser with passkey support.",
    });
  }
  if (name === "NotAllowedError" && elapsedMs < REFUSED_AT_ONCE_MS) {
    return new HostCallError({
      code: "not-allowed",
      message: "Passkeys are not available in this browser.",
      next: "Check your browser settings.",
    });
  }
  if (name === "NotAllowedError" || name === "AbortError") {
    return new HostCallError({
      code: "needs-sign-in",
      message: "The passkey prompt was closed or timed out.",
      next: "Try again.",
    });
  }
  return new HostCallError({
    code: "needs-sign-in",
    message: "This browser could not use a passkey.",
    next: "Try again.",
  });
}

async function passkey(kind: "setup" | "login", args: Record<string, unknown>): Promise<SessionReply> {
  if (hasPendingLogout()) {
    const cleared = await retryPendingLogout();
    if (!cleared) {
      throw new HostCallError({
        code: "unreachable",
        message: "Still finishing your previous sign-out.",
        next: "Reconnect and try again.",
      });
    }
  }
  const start = await hostJson<Ceremony>(`/v1/auth/${kind}/start`, args);
  const t0 = performance.now();
  let credential: PublicKeyCredential | null;
  try {
    credential = (
      kind === "setup"
        ? await navigator.credentials.create({
            publicKey: PublicKeyCredential.parseCreationOptionsFromJSON(
              start.options.publicKey as Parameters<typeof PublicKeyCredential.parseCreationOptionsFromJSON>[0],
            ),
          })
        : await navigator.credentials.get({
            publicKey: PublicKeyCredential.parseRequestOptionsFromJSON(
              start.options.publicKey as Parameters<typeof PublicKeyCredential.parseRequestOptionsFromJSON>[0],
            ),
          })
    ) as PublicKeyCredential | null;
  } catch (err) {
    throw mapPasskeyError(err, performance.now() - t0);
  }
  if (!credential) {
    throw new HostCallError({ code: "needs-sign-in", message: "No passkey was chosen.", next: "Try again." });
  }
  const result = await hostJson<SessionReply>(`/v1/auth/${kind}/finish`, {
    ceremony: start.ceremony,
    credential: credential.toJSON(),
  });
  saveSession(result.session);
  return result;
}

/** Sign in with a discoverable passkey. The host sets the strict HttpOnly cookie. */
export async function signInHost(): Promise<SessionInfo> {
  return (await passkey("login", {})).session;
}

/** Enroll the first owner's passkey. The caller displays returned recovery codes once. */
export async function setupHost(token: string, name: string): Promise<SessionReply> {
  return passkey("setup", { token: token.trim(), name: name.trim() });
}

/** Revoke the current installation and clear account-scoped storage (ADR 0030). */
export async function signOutHost(flushPending?: () => Promise<void>): Promise<{ offline: boolean }> {
  if (flushPending) {
    await Promise.race([
      flushPending().catch(() => {}),
      new Promise<void>((resolve) => setTimeout(resolve, 3000)),
    ]);
  }
  let offline = false;
  try {
    await hostJson<void>("/v1/auth/logout", {});
    setPendingLogout(false);
  } catch {
    offline = true;
    setPendingLogout(true);
  }
  clearAccountStorage();
  broadcastSignOut();
  return { offline };
}

/** Get the installed declarations reachable by the signed-in person. */
export function fetchDeclarations(session: SessionInfo): Promise<SpaceDeclarations> {
  return hostCall(session, "s.declarations");
}

/** Invoke the single host call route: `POST /v1/spaces/{space}/call`. */
export async function hostCall<T = unknown>(
  session: SessionInfo,
  call: string,
  input: Record<string, unknown> = {},
  idempotencyKey?: string,
  slotSelection?: SlotSelection,
): Promise<T> {
  const body: Record<string, unknown> = { call, input };
  if (idempotencyKey) body.idempotency_key = idempotencyKey;
  if (slotSelection) body.slot_selection = slotSelection;
  const { result } = await hostJson<{ result: T }>(`/v1/spaces/${session.space}/call`, body);
  return result;
}

/** Convenience wrapper for `s.read`. */
export function readItem(session: SessionInfo, item: string): Promise<ItemDetail> {
  return hostCall<ItemDetail>(session, "s.read", { item });
}

/** Connection lifecycle state of [`LiveClient`]. */
export type ConnectionState = "connecting" | "open" | "reconnecting" | "closed";

interface QueryListener {
  kind: "query";
  spec: QuerySpec;
  onRows: (rows: RowData[], totals: Record<string, unknown>) => void;
}

interface RecordsListener {
  kind: "records";
  item: string;
  lastAnchor: string | null;
  onSnapshot: (records: RecordData[], meta: LiveItemMetadata) => void;
  onDelta: (records: RecordData[], meta: LiveItemMetadata) => void;
  onAppend: (record: RecordData, version: number) => void;
  onItemChange: (meta: LiveItemMetadata) => void;
}

type Listener = QueryListener | RecordsListener;

/**
 * Single WebSocket per browser session carrying all active query and record
 * subscriptions. Automatically reconnects and resumes record streams from each
 * subscription's last applied anchor so no record is duplicated or reordered.
 */
export class LiveClient {
  private readonly session: SessionInfo;
  private ws: WebSocket | null = null;
  private ready = false;
  private closed = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectDelay = 250;
  private nextId = 1;
  private subs = new Map<string, Listener>();
  private stateListener?: (state: ConnectionState) => void;

  constructor(session: SessionInfo, onState?: (state: ConnectionState) => void) {
    this.session = session;
    this.stateListener = onState;
    this.connect();
  }

  private wsUrl(): string {
    const proto = location.protocol === "https:" ? "wss:" : "ws:";
    return `${proto}//${location.host}/v1/spaces/${this.session.space}/live`;
  }

  private connect(): void {
    if (this.closed) return;
    this.stateListener?.(this.ws ? "reconnecting" : "connecting");
    const ws = new WebSocket(this.wsUrl());
    this.ws = ws;
    this.ready = false;

    ws.addEventListener("message", (ev) => {
      if (this.ws !== ws || this.closed) return;
      let frame: LiveServerFrame;
      try {
        frame = JSON.parse(String(ev.data)) as LiveServerFrame;
      } catch {
        return;
      }
      if (frame.type === "ready") {
        this.ready = true;
        this.reconnectDelay = 250;
        this.stateListener?.("open");
        for (const [id, listener] of this.subs) {
          this.sendSubscribe(id, listener);
        }
        return;
      }
      if (frame.type === "rows") {
        const sub = this.subs.get(frame.sub);
        if (sub?.kind === "query") sub.onRows(frame.rows, frame.totals);
        return;
      }
      if (frame.type === "records") {
        const sub = this.subs.get(frame.sub);
        if (sub?.kind === "records") {
          sub.lastAnchor = frame.last_anchor;
          const { version, fields, computed, state, when } = frame;
          const meta = { version, fields, computed, state, when };
          if (frame.mode === "delta") sub.onDelta(frame.records, meta);
          else sub.onSnapshot(frame.records, meta);
        }
        return;
      }
      if (frame.type === "record_append") {
        const sub = this.subs.get(frame.sub);
        if (sub?.kind === "records") {
          sub.lastAnchor = frame.record.anchor;
          sub.onAppend(frame.record, frame.version);
        }
        return;
      }
      if (frame.type === "item_change") {
        const sub = this.subs.get(frame.sub);
        if (sub?.kind === "records") {
          const { version, fields, computed, state, when } = frame;
          sub.onItemChange({ version, fields, computed, state, when });
        }
      }
    });

    ws.addEventListener("close", () => {
      if (this.ws !== ws) return;
      this.ready = false;
      if (this.closed) {
        this.stateListener?.("closed");
        return;
      }
      this.stateListener?.("reconnecting");
      const delay = this.reconnectDelay;
      this.reconnectDelay = Math.min(delay * 2, 3000);
      this.reconnectTimer = setTimeout(() => this.connect(), delay);
    });
  }

  private send(payload: Record<string, unknown>): void {
    if (this.ws && this.ready && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(payload));
    }
  }

  private sendSubscribe(id: string, listener: Listener): void {
    if (listener.kind === "query") {
      this.send({ type: "subscribe", sub: id, query: listener.spec });
    } else {
      const records: Record<string, unknown> = { item: listener.item };
      if (listener.lastAnchor) records.after = listener.lastAnchor;
      this.send({ type: "subscribe", sub: id, records });
    }
  }

  /** Subscribe to a collection or ad-hoc query; returns an unsubscribe callback. */
  subscribeQuery(spec: QuerySpec, onRows: (rows: RowData[], totals: Record<string, unknown>) => void): () => void {
    const id = `q${this.nextId++}`;
    const listener: QueryListener = { kind: "query", spec, onRows };
    this.subs.set(id, listener);
    this.sendSubscribe(id, listener);
    return () => {
      this.subs.delete(id);
      this.send({ type: "unsubscribe", sub: id });
    };
  }

  /** Subscribe to one item's live records stream; returns an unsubscribe callback. */
  subscribeRecords(
    item: string,
    handlers: Omit<RecordsListener, "kind" | "item" | "lastAnchor">,
    initialAfter: string | null = null,
  ): () => void {
    const id = `r${this.nextId++}`;
    const listener: RecordsListener = { kind: "records", item, lastAnchor: initialAfter, ...handlers };
    this.subs.set(id, listener);
    this.sendSubscribe(id, listener);
    return () => {
      this.subs.delete(id);
      this.send({ type: "unsubscribe", sub: id });
    };
  }

  /** Force-drop the underlying WebSocket (for reconnect tests) and let it auto-reconnect from `lastAnchor`. */
  forceReconnect(): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.close(4000, "test-reconnect");
    }
  }

  /** Close the session WebSocket permanently. */
  close(): void {
    this.closed = true;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close(1000, "sign-out");
    this.ws = null;
    this.subs.clear();
  }
}
