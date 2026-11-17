/**
 * Reactive state for the Threads app: the thread list, the open requests and the provider list, kept current by watching the client.
 * One instance per page. The client is the only thing that changes when the real operations arrive.
 */
import type { ThreadsClient } from "./client";
import { FixtureClient } from "./fixtures";
import type { ProviderOption, Request, ThreadSummary } from "./model";

/** The client in use. Replace with the real one when it exists. */
export const client: ThreadsClient = new FixtureClient();

/** Threads, requests and providers as runes. */
class ThreadsStore {
  threads = $state<ThreadSummary[]>([]);
  requests = $state<Request[]>([]);
  providers = $state<ProviderOption[]>([]);
  machines = $state<string[]>([]);
  ready = $state(false);

  /** Loads and subscribes. Safe to call once at start. */
  async start(): Promise<void> {
    const [t, r, p, m] = await Promise.all([client.listThreads(), client.requests(), client.providers(), client.machines()]);
    this.threads = t;
    this.requests = r;
    this.providers = p;
    this.machines = m;
    this.ready = true;
    client.watchThreads((x) => (this.threads = x));
    client.watchRequests((x) => (this.requests = x));
  }

  /** Reloads the list and the open requests (pull to refresh). */
  async refresh(): Promise<void> {
    [this.threads, this.requests] = await Promise.all([client.listThreads(), client.requests()]);
  }

  /** One thread by id. */
  get(id: string): ThreadSummary | undefined {
    return this.threads.find((t) => t.id === id);
  }
  /** Children of a thread, newest first. */
  childrenOf(id: string): ThreadSummary[] {
    return this.threads.filter((t) => t.parentId === id).sort((a, b) => b.updatedAt - a.updatedAt);
  }
  /** Top-level threads grouped by workspace, each group newest first. */
  get groups(): { workspace: string; threads: ThreadSummary[] }[] {
    const by = new Map<string, ThreadSummary[]>();
    for (const t of this.threads) if (!t.parentId) (by.get(t.workspace) ?? by.set(t.workspace, []).get(t.workspace)!).push(t);
    return [...by].map(([workspace, threads]) => ({ workspace, threads: threads.sort((a, b) => b.updatedAt - a.updatedAt) }));
  }
  /**
   * Threads with something waiting on the person, newest request first, with what they are waiting on. The attention card shows the first three; the
   * thread list leaves those out so a thread is never shown twice.
   */
  get attention(): { thread: ThreadSummary; kinds: Request["kind"][] }[] {
    const by = new Map<string, Request["kind"][]>();
    for (const r of this.requests) (by.get(r.threadId) ?? by.set(r.threadId, []).get(r.threadId)!).push(r.kind);
    return [...by].flatMap(([id, kinds]) => { const thread = this.get(id); return thread ? [{ thread, kinds }] : []; });
  }
  /** Open requests for one thread (a parent also sees its children's). */
  requestsFor(id: string): Request[] {
    const ids = new Set([id, ...this.threads.filter((t) => t.parentId === id).map((t) => t.id)]);
    return this.requests.filter((r) => ids.has(r.threadId));
  }
}

/** The shared store. */
export const store = new ThreadsStore();
