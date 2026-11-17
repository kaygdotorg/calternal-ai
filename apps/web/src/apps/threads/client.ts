/**
 * The one interface the Threads app talks to. `FixtureClient` (fixtures.ts) implements it today; the real client, built on the
 * generated operations (`threads.list`, `threads.send` and the rest of `contract/operations.json`), plugs in here and no component changes.
 * Every call is async because the real one crosses the network. Reads are bounded: a thread is read in message ranges, never whole.
 */
import type { Answer, Change, Message, Permission, Step, ProviderOption, Request, RunInfo, RunsOn, ThreadSummary } from "./model";

/** What a watcher is told about a thread while it is open. */
export interface ThreadEvents {
  /** A complete message was appended (a sent message, or an agent message that finished streaming). */
  onMessage(message: Message, index: number): void;
  /** A streaming agent message started. Its text arrives through `onDelta` and ends with `onMessage` for the same ID. */
  onStreamStart(messageId: string): void;
  /** More text for the streaming message. Called often; must stay cheap. A client that is told the person does not want streaming may skip it. */
  onDelta(text: string): void;
  /** The agent started another tool step. The thread shows a working state and these steps while it runs, then the finished message at once. */
  onStep(step: Step): void;
  /** The thread's summary changed (state, title, provider). */
  onSummary(summary: ThreadSummary): void;
}

/** The Threads app's whole data surface. */
export interface ThreadsClient {
  /** The providers and models offered. */
  providers(): Promise<ProviderOption[]>;
  /** All threads the person can see in the current space, children included (`parentId`). */
  listThreads(): Promise<ThreadSummary[]>;
  /** Subscribes to changes of the thread list (state, new threads). Returns the unsubscribe. */
  watchThreads(cb: (threads: ThreadSummary[]) => void): () => void;
  /** Messages `from` (inclusive) to `to` (exclusive), oldest first. At most a few hundred per call. */
  messages(threadId: string, from: number, to: number): Promise<Message[]>;
  /** Follows a thread while it is open. Returns the unsubscribe. */
  watch(threadId: string, events: ThreadEvents): () => void;
  /** Sends a message (steers a running turn). `requestId` makes a retry safe. */
  send(threadId: string, text: string, requestId: string): Promise<void>;
  /** Stops the running turn. */
  stop(threadId: string): Promise<void>;
  /** Changes the provider and model for the next message. Nothing visible happens in the thread. */
  setProvider(threadId: string, provider: ThreadSummary["provider"], model: string): Promise<void>;
  /** Renames a thread. */
  rename(threadId: string, title: string): Promise<void>;
  /** Archives a thread (and its finished children): it leaves the list and keeps its file. */
  archive(threadId: string): Promise<void>;
  /** Takes back the last archive of this thread (and the finished children that went with it). Resolves false if it cannot. */
  unarchive(threadId: string): Promise<boolean>;
  /** How much the agent may do without asking, for the next message. */
  setPermission(threadId: string, permission: Permission): Promise<void>;
  /** The machines this person can run a thread on (their own, and the pool if their group allows it). */
  machines(): Promise<string[]>;
  /** Where the thread runs next: a machine, sandboxed or direct. Never moves work that is already running. */
  setRunsOn(threadId: string, runsOn: RunsOn): Promise<void>;
  /** Open requests across the space, newest first. */
  requests(): Promise<Request[]>;
  /** Subscribes to changes of the open requests. */
  watchRequests(cb: (requests: Request[]) => void): () => void;
  /** Answers a request. For a secret the value leaves the client here and is not kept. */
  answer(requestId: string, answer: Answer): Promise<void>;
  /** The Changes tab. */
  changes(threadId: string): Promise<Change[]>;
  /** The Run tab. */
  run(threadId: string): Promise<RunInfo>;
}
