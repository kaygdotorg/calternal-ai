/**
 * Reactive state for the Threads app: the thread list, the open requests and the provider list, kept current by watching the client.
 * One instance per page. The client is the only thing that changes when the real operations arrive.
 */
import { inboxOf, snoozedOf, type InboxItem, type SnoozedItem } from "./inbox";
import type { SignInRequest } from "./model";
import type { ThreadsClient } from "./client";
import { FixtureClient } from "./fixtures";
import { server, serverMode } from "../../server/client";
import { ServerThreads } from "../../server/threads-client";
import type { ProviderOption, Request, ThreadSummary } from "./model";

/** The client in use: the real server with `?server=1` (development token mode, see server/client.ts), the fixtures otherwise (the default, the preview and the tests). */
export const client: ThreadsClient = serverMode() ? new ServerThreads(server) : new FixtureClient();

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
    this.setRequests(r);
    this.providers = p;
    this.machines = m;
    this.ready = true;
    client.watchThreads((x) => (this.threads = x));
    client.watchRequests((x) => this.setRequests(x));
  }

  /** Reloads the list and the open requests (pull to refresh). */
  async refresh(): Promise<void> {
    const [t, r] = await Promise.all([client.listThreads(), client.requests()]);
    this.threads = t;
    this.setRequests(r);
  }

  /** Sign-ins that ended, kept for a few seconds so the conversation can say so ("Authentication done"); never stored anywhere (the thread file carries one line). */
  signedIn = $state<{ req: SignInRequest; state: "done" | "failed" }[]>([]);

  /** Takes the server's open requests; a sign-in that has left the list is looked up once so its item can say how it ended. */
  private setRequests(next: Request[]): void {
    const gone = this.requests.filter((r): r is SignInRequest => r.kind === "signin" && !next.some((n) => n.id === r.id));
    this.requests = next;
    for (const r of gone) void this.settle(r);
  }
  private async settle(r: SignInRequest): Promise<void> {
    let st: "pending" | "done" | "failed" | "gone" = "gone";
    try { st = (await client.signInStatus?.(r.id)) ?? "gone"; } catch { /* unknown: nothing to say */ }
    if (st !== "done" && st !== "failed") return;
    this.signedIn = [...this.signedIn.filter((f) => f.req.id !== r.id), { req: r, state: st }];
    setTimeout(() => (this.signedIn = this.signedIn.filter((f) => f.req.id !== r.id)), st === "done" ? 8000 : 30000);
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
  /** The clock the Inbox and Snoozed are worked out at; the sidebar moves it to the next time a snoozed thread returns, so a thread comes back by itself. */
  now = $state(Date.now());
  /** The Inbox: the threads that need you and are not snoozed, and why (inbox.ts). */
  get inbox(): InboxItem[] {
    return inboxOf(this.threads, this.requests, this.now);
  }
  /** The snoozed threads, the soonest to return first. */
  get snoozed(): SnoozedItem[] {
    return snoozedOf(this.threads, this.now);
  }
  /** Open requests for one thread (a parent also sees its children's). */
  requestsFor(id: string): Request[] {
    const ids = new Set([id, ...this.threads.filter((t) => t.parentId === id).map((t) => t.id)]);
    return this.requests.filter((r) => ids.has(r.threadId));
  }
}

/** The shared store. */
export const store = new ThreadsStore();

/** Starts an empty thread (palette "New thread"), refreshes the list so the sidebar has it, and returns it; the caller opens it. */
export async function startThread(title?: string): Promise<ThreadSummary> {
  const t = await client.createThread(title);
  await store.refresh();
  return t;
}
