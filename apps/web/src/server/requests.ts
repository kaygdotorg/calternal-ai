/**
 * Requests on the real server: questions, approvals and secret requests (the request bar and the attention card), through the unified request operations of
 * issue #32: `requests.list`, `requests.answer`, `requests.approve` (lifetime `once` or `thread`) and `requests.deny`. The wire shape below is the lane's stated goal
 * (list / answer / approve / deny); it is NOT yet in dev, so the mapping here (`requestOf`, `answerInput`) is the one place to change when the contract lands. A secret's value leaves
 * the client in `requests.answer` and is not kept; a failure is thrown as it came (`ServerError`). The list is polled every 4 s while watched (a feed replaces the poll).
 */
import { spaces } from "../shell/spaces.svelte";
import type { RequestsClient } from "../apps/threads/client";
import type { Answer, Request } from "../apps/threads/model";
import { ServerClient, ServerError } from "./client";

/** One open request as the server lists it (assumed shape, see the file header). */
export interface WireRequest {
  id: string;
  kind: "question" | "approval" | "secret";
  thread_id: string;
  title?: string;
  /** question: the form in the generated-UI language. */
  form?: string;
  /** approval: what the agent wants, and the exact command. */
  action?: string;
  command?: string;
  /** secret: the manifest name, label, reason, scope, whether the turn needs it, and when it expires (ms). */
  name?: string;
  label?: string;
  why?: string;
  scope?: "personal" | "space";
  required?: boolean;
  expires_ms?: number;
}

/** A listed request as the Threads app's type. An unknown kind is dropped (the bar cannot draw it). */
export function requestOf(w: WireRequest): Request | null {
  switch (w.kind) {
    case "question": return { id: w.id, kind: "question", threadId: w.thread_id, title: w.title ?? "A question", form: w.form ?? "" };
    case "approval": return { id: w.id, kind: "approval", threadId: w.thread_id, title: w.title ?? "Approval needed", action: w.action ?? "", command: w.command ?? "" };
    case "secret": return { id: w.id, kind: "secret", threadId: w.thread_id, name: w.name ?? "", label: w.label ?? w.name ?? "", why: w.why ?? "", scope: w.scope ?? "personal", required: w.required ?? true, expiresAt: w.expires_ms ?? 0 };
    default: return null;
  }
}

/** The operation and input an answer becomes. */
export function answerInput(id: string, a: Answer): { operation: string; input: Record<string, unknown> } {
  switch (a.kind) {
    case "question": return { operation: "requests.answer", input: { id, values: a.values } };
    case "approval": return a.decision === "deny" ? { operation: "requests.deny", input: { id } } : { operation: "requests.approve", input: { id, lifetime: a.decision } };
    case "secret": return { operation: "requests.answer", input: { id, secret: a.value } };
    case "secret-skip": return { operation: "requests.deny", input: { id } };
  }
}

const EVERY = 4000;

export class ServerRequests implements RequestsClient {
  private watchers = new Set<(r: Request[]) => void>();
  private timer = 0;
  constructor(private server: ServerClient, private space: () => string = () => spaces.current.id) {}

  /** Set when the server answered `not_found` to `requests.list`: it does not offer requests yet, so there is nothing to ask again until the page is reloaded. */
  private absent = false;
  async requests(): Promise<Request[]> {
    if (this.absent) return [];
    try {
      const { requests } = await this.server.operate<{ requests: WireRequest[] }>("requests.list", { space_id: this.space() });
      return requests.flatMap((w) => requestOf(w) ?? []);
    } catch (e) {
      if (e instanceof ServerError && e.code === "not_found") { this.absent = true; return []; }
      throw e;
    }
  }
  watchRequests(cb: (r: Request[]) => void): () => void {
    this.watchers.add(cb);
    if (!this.timer) this.timer = window.setInterval(() => { if (!document.hidden && !this.absent) void this.refresh(); }, EVERY);
    return () => { this.watchers.delete(cb); if (!this.watchers.size) { clearInterval(this.timer); this.timer = 0; } };
  }
  /** Re-reads and tells every watcher (after an answer, and on each tick). */
  async refresh(): Promise<void> {
    try { const r = await this.requests(); for (const w of this.watchers) w(r); } catch { /* offline: the next tick tries again */ }
  }
  async answer(id: string, a: Answer): Promise<void> {
    const { operation, input } = answerInput(id, a);
    await this.server.operate(operation, input);
    await this.refresh();
  }
}
