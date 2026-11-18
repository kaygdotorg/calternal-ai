/**
 * Requests on the real server: questions, approvals and secret prompts (the request bar and the attention card), through the requests lane's operations (`docs/requests.md`, `crates/core/src/requests.rs`):
 *   - `requests.list` `{}` -> `{ requests: [{ id, space_id|null, thread_id|null, detail }] }`, account-wide; `detail.kind` is `question` (`label`, `form: { genui_version: 2, source }`),
 *     `approval` (`app`, `write`, `action: { operation, input, request_id, risk }`) or `secret` (`name`, `label`, `reason`, `expires_at_ms`).
 *   - `requests.answer` `{ id, answer }` (a question's form values), `requests.approve` `{ id, lifetime: "once" | "this_thread" }`, `requests.deny` `{ id }`.
 *   - A secret's value is not an operation: it goes to `POST /api/v1/auth/secrets/provide` `{ space_id?, request_id, value }` (a signed-in session with a fresh passkey check), and is not kept here.
 * Approving needs a fresh passkey proof on an installation session, so a development bearer is refused: the refusal reaches the person as the server's own words.
 * Which lifetimes an approval allows is the server's rule (irreversible and access-widening actions are single-use): `lifetimesOf` reads it from the action's risk (or a `lifetimes` list when the server sends one),
 * and the bar offers nothing else. Not polled: the thread feed calls `refresh` when a turn starts or stops waiting.
 */
import { spaces } from "../shell/spaces.svelte";
import type { RequestsClient } from "../apps/threads/client";
import type { Answer, Request } from "../apps/threads/model";
import { ServerClient, ServerError } from "./client";
import { reverify as reverifyPasskey } from "./auth";

/** One request as the server lists it. */
export interface WireRequest {
  /** The request's UUID; answering it names it. */
  id: string;
  /** The space it belongs to, when it has one. */
  space_id?: string | null;
  /** The thread that asked, when it came from a thread. */
  thread_id?: string | null;
  /** What is asked: a question, an approval or a secret, with the data its form needs (read by `requestOf`). */
  detail:
    | { kind: "question"; label: string; form: { genui_version: number; source: string } }
    | { kind: "approval"; app: string; write: boolean; action?: { operation?: string; input?: unknown; request_id?: string; risk?: string }; lifetimes?: string[] }
    | { kind: "secret"; name: string; label: string; reason: string; expires_at_ms: number };
}

/** The lifetimes an approval may be given for. A server-sent list wins; otherwise the action's risk decides (the requests lane's rule). */
export function lifetimesOf(d: Extract<WireRequest["detail"], { kind: "approval" }>): ("once" | "thread")[] {
  if (d.lifetimes?.length) return d.lifetimes.map((l) => (l === "this_thread" || l === "thread" ? "thread" : "once")).filter((l, i, a) => a.indexOf(l) === i) as ("once" | "thread")[];
  const risk = d.action?.risk;
  return risk === "irreversible" || risk === "widens_access" ? ["once"] : ["once", "thread"];
}

/** A listed request as the Threads app's type. An unknown kind is dropped (the bar cannot draw it). */
export function requestOf(w: WireRequest): Request | null {
  const threadId = w.thread_id ?? "";
  const d = w.detail;
  switch (d?.kind) {
    case "question": return { id: w.id, kind: "question", threadId, title: d.label, form: d.form?.source ?? "" };
    case "approval": {
      const op = d.action?.operation;
      const input = d.action?.input;
      return {
        id: w.id, kind: "approval", threadId, title: "Approval needed",
        action: op ? `The agent wants to run ${op}` : `The agent asks for ${d.write ? "write" : "read"} access to ${d.app}`,
        command: op ? `${op} ${input === undefined ? "" : JSON.stringify(input)}`.trim().slice(0, 600) : `${d.app} (${d.write ? "read and write" : "read"})`,
        lifetimes: lifetimesOf(d),
      };
    }
    case "secret": return { id: w.id, kind: "secret", threadId, name: d.name, label: d.label || d.name, why: d.reason, scope: w.space_id ? "space" : "personal", spaceId: w.space_id ?? undefined, required: true, expiresAt: d.expires_at_ms };
    default: return null;
  }
}

/** The operation and input an answer becomes; null when it needs none (a skipped secret has no server call: the prompt just expires). */
export function answerInput(id: string, a: Answer): { operation: string; input: Record<string, unknown> } | null {
  switch (a.kind) {
    case "question": return { operation: "requests.answer", input: { id, answer: a.values } };
    case "approval": return a.decision === "deny" ? { operation: "requests.deny", input: { id } } : { operation: "requests.approve", input: { id, lifetime: a.decision === "thread" ? "this_thread" : "once" } };
    default: return null;
  }
}

/** The real server's requests (questions, approvals, secrets) behind the requests interface: lists them for a space and answers; failures are `ServerError`, and an answer to a request that is gone is `not_found`. One read per call. */
export class ServerRequests implements RequestsClient {
  private watchers = new Set<(r: Request[]) => void>();
  /** Prompts the person chose to skip on this page: hidden until the page is reloaded (the server has nothing to decline; the prompt expires by itself). */
  private skipped = new Set<string>();
  private known = new Map<string, Request>();
  /** Set when the server answered `not_found` to `requests.list`: it does not offer requests, so there is nothing to ask again until the page is reloaded. */
  private absent = false;
  /** `reverify` asks for the passkey again (a real ceremony in a browser); tests pass a stand-in. */
  constructor(private server: ServerClient, private space: () => string = () => spaces.current.id, private reverify: () => Promise<void> = reverifyPasskey) { void this.space; }
  /**
   * Runs a call that needs a fresh passkey proof (an approval, a secret's value). The server answers a stale session with a plain `forbidden`, not a
   * dedicated code, so a forbidden answer is retried once after asking for the passkey; if the second try is refused too, that refusal is thrown.
   * Cost: none unless the proof is stale; then one passkey ceremony (two requests and the person's touch).
   */
  private async fresh<T>(call: () => Promise<T>): Promise<T> {
    try { return await call(); } catch (e) {
      if (!(e instanceof ServerError) || e.code !== "forbidden") throw e;
      await this.reverify();
      return call();
    }
  }

  async requests(): Promise<Request[]> {
    if (this.absent) return [];
    try {
      const { requests } = await this.server.operate<{ requests: WireRequest[] }>("requests.list", {});
      const out = requests.flatMap((w) => requestOf(w) ?? []).filter((r) => !this.skipped.has(r.id));
      this.known = new Map(out.map((r) => [r.id, r]));
      return out;
    } catch (e) {
      if (e instanceof ServerError && e.code === "not_found") { this.absent = true; return []; }
      throw e;
    }
  }
  /** Watchers hear a new list whenever `refresh` runs: after an answer, and when the thread feed says a turn started or stopped waiting. Nothing is polled. */
  watchRequests(cb: (r: Request[]) => void): () => void {
    this.watchers.add(cb);
    return () => { this.watchers.delete(cb); };
  }
  /** Re-reads and tells every watcher. */
  async refresh(): Promise<void> {
    if (this.absent || !this.watchers.size) return;
    try { const r = await this.requests(); for (const w of this.watchers) w(r); } catch { /* the server is away: the next change tries again */ }
  }
  async answer(id: string, a: Answer): Promise<void> {
    if (a.kind === "secret") await this.provide(id, a.value);
    else if (a.kind === "secret-skip") this.skipped.add(id);
    else { const call = answerInput(id, a)!; const run = () => this.server.operate(call.operation, call.input); await (call.operation === "requests.approve" ? this.fresh(run) : run()); }
    await this.refresh();
  }
  /** Sends a secret's value to the trusted endpoint. The value is not kept; a refusal (no fresh passkey check, expired) is thrown as the server said it. */
  private async provide(id: string, value: string): Promise<void> {
    const r = this.known.get(id);
    await this.fresh(() => this.server.post("/api/v1/auth/secrets/provide", { request_id: id, ...(r?.kind === "secret" && r.spaceId ? { space_id: r.spaceId } : {}), value }));
  }
}
