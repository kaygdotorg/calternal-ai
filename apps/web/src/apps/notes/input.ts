/**
 * Private input that outlives a page: drafts (unsent text per item, complete replacement, never shared) and pending submissions (writes queued while the server could not be
 * reached, with their original creation time). Two implementations behind one interface: fixtures (in memory, the default) and the server (`drafts.*`, `pending.*` and the
 * account feed, docs/account-input.md). A queued write older than seven days is `confirmation_required`: it is never sent automatically.
 */
import { ServerClient, ServerError } from "../../server/client";

export interface Draft { spaceId: string; itemId: string; text: string; revision: string }
export interface PendingWrite {
  /** The underlying request's id (stable; a retry reuses it). */
  id: string;
  spaceId: string;
  itemId: string;
  createdAtMs: number;
  operation: string;
  input: Record<string, unknown>;
  /** `queued`, or `confirmation_required` once it is more than seven days old. */
  state: string;
}
export interface InputClient {
  draft(spaceId: string, itemId: string): Promise<Draft | null>;
  saveDraft(spaceId: string, itemId: string, text: string): Promise<void>;
  dropDraft(spaceId: string, itemId: string): Promise<void>;
  pending(): Promise<PendingWrite[]>;
  /** Queues a write; resolves with the queued record. */
  queue(w: Omit<PendingWrite, "state">): Promise<PendingWrite>;
  /** Sends a queued write now (`confirm` for one that needs the person's confirmation). Resolves when sent; rejects if the server still cannot be reached. */
  send(id: string): Promise<void>;
  cancel(id: string): Promise<void>;
  /** Calls `onChange` when drafts or pending input changed elsewhere (the account feed). Returns the stop function. */
  watch(onChange: () => void): () => void;
}

const WEEK = 7 * 24 * 3600 * 1000;

/** The in-memory fixture. `send` goes through the `apply` the app gives (the fixture's own write), so the flow can be tried without a server. */
export class FixtureInput implements InputClient {
  private drafts = new Map<string, Draft>();
  private queue_ = new Map<string, PendingWrite>();
  private listeners = new Set<() => void>();
  constructor(private apply: (w: PendingWrite) => Promise<void> = async () => {}) {}
  private ping() { for (const l of this.listeners) l(); }
  async draft(spaceId: string, itemId: string) { return this.drafts.get(`${spaceId}/${itemId}`) ?? null; }
  async saveDraft(spaceId: string, itemId: string, text: string) { this.drafts.set(`${spaceId}/${itemId}`, { spaceId, itemId, text, revision: String(Date.now()) }); }
  async dropDraft(spaceId: string, itemId: string) { this.drafts.delete(`${spaceId}/${itemId}`); }
  async pending() { return [...this.queue_.values()].map((w) => (Date.now() - w.createdAtMs > WEEK ? { ...w, state: "confirmation_required" } : w)); }
  async queue(w: Omit<PendingWrite, "state">) { const r = { ...w, state: "queued" }; this.queue_.set(w.id, r); this.ping(); return r; }
  async send(id: string) {
    const w = this.queue_.get(id);
    if (!w) return;
    await this.apply(w);
    this.queue_.delete(id);
    this.ping();
  }
  async cancel(id: string) { this.queue_.delete(id); this.ping(); }
  watch(onChange: () => void) { this.listeners.add(onChange); return () => this.listeners.delete(onChange); }
}

/** The real server's account input. */
export class ServerInput implements InputClient {
  constructor(private server: ServerClient) {}
  async draft(spaceId: string, itemId: string): Promise<Draft | null> {
    const r = (await this.server.drafts.get(spaceId, itemId)).record;
    return r ? { spaceId: r.space_id, itemId: r.item_id, text: r.text ?? "", revision: r.revision } : null;
  }
  async saveDraft(spaceId: string, itemId: string, text: string) { await this.server.drafts.save(spaceId, itemId, text); }
  async dropDraft(spaceId: string, itemId: string) { await this.server.drafts.delete(spaceId, itemId); }
  async pending(): Promise<PendingWrite[]> {
    return (await this.server.pending.list()).records.flatMap((r) => (r.request ? [{ id: r.id, spaceId: r.space_id, itemId: r.item_id, createdAtMs: r.created_at_ms, operation: r.request.operation, input: r.request.input as Record<string, unknown>, state: r.state }] : []));
  }
  async queue(w: Omit<PendingWrite, "state">): Promise<PendingWrite> {
    const r = (await this.server.pending.queue(w.spaceId, w.itemId, w.createdAtMs, { request_id: w.id, operation: w.operation, input: w.input })).record;
    return { ...w, state: r?.state ?? "queued" };
  }
  async send(id: string) {
    const cur = (await this.pending()).find((p) => p.id === id);
    // An old record asks to be confirmed (the person pressed Send: that is the confirmation); a fresh one is submitted.
    if (cur?.state === "confirmation_required") await this.server.pending.confirm(id);
    else await this.server.pending.submit(id);
  }
  async cancel(id: string) { await this.server.pending.cancel(id); }
  watch(onChange: () => void): () => void {
    const ac = new AbortController();
    void (async () => {
      let bookmark: string | undefined;
      for (let tries = 0; !ac.signal.aborted && tries < 5; tries++) {
        let reset = false;
        try {
          await this.server.accountLive((f) => {
            if (f === "Readiness") return; // only asked for by the Machines page
            if (f === "Reset") { reset = true; bookmark = undefined; onChange(); }
            else if ("Changed" in f) { onChange(); bookmark = f.Changed.bookmark; }
            else bookmark = f.Ready.bookmark;
          }, { bookmark, signal: ac.signal });
        } catch (e) { if (!(e instanceof ServerError)) break; }
        if (!reset) break;
      }
    })();
    return () => ac.abort();
  }
}
