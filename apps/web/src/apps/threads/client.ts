/**
 * The one interface the Threads app talks to. `FixtureClient` (fixtures.ts) implements it today; the real client, built on the
 * generated operations (`threads.list`, `threads.send` and the rest of `contract/operations.json`), plugs in here and no component changes.
 * Every call is async because the real one crosses the network. Reads are bounded: a thread is read in message ranges, never whole.
 */
import type { Answer, Attachment, Change, Message, MessageHit, Permission, Step, ProviderOption, Request, RunInfo, RunsOn, ThreadSummary } from "./model";

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
  /** What the running turn is doing now, in words for the working row ("Waiting to start", "Starting the machine", "Thinking", "Running ls", "Writing"). */
  onPhase?(phase: string): void;
  /** The provider is reasoning: more of its reasoning text (may be empty when the provider shares only that it is thinking), and `done` when it has finished, with how long it took in milliseconds. */
  onThought?(thought: { text: string; done?: boolean; ms?: number }): void;
  /** The thread's summary changed (state, title, provider). */
  onSummary(summary: ThreadSummary): void;
  /** The messages from some index on are a different version now (an edit, or another version chosen): forget what was read and start from `count` messages. */
  onReset?(count: number): void;
  /** The working turn ended without a message of its own appearing (stopped, failed): the working state goes away. */
  onStreamEnd?(): void;
}

/** The composer's choices: which provider and model, how much the agent may do, where it runs. They apply to the next message (on the server they travel with `send`). */
export interface ChoicesClient {
  /** The providers and models offered. */
  providers(): Promise<ProviderOption[]>;
  /** The reasoning level for the next message (one of the model's levels), or null for the provider's default. */
  setReasoning(threadId: string, level: string | null): Promise<void>;
  /** Changes the provider and model for the next message. Nothing visible happens in the thread. */
  setProvider(threadId: string, provider: ThreadSummary["provider"], model: string): Promise<void>;
  /** How much the agent may do without asking, for the next message. */
  setPermission(threadId: string, permission: Permission): Promise<void>;
  /** The machines this person can run a thread on (their own, and the pool if their group allows it). */
  machines(): Promise<string[]>;
  /** Asks the machine to check the thread's provider again (is it installed, is the login there), then re-reads readiness. */
  check(threadId: string): Promise<void>;
  /** Where the thread runs next: a machine, sandboxed or direct. Never moves work that is already running. */
  setRunsOn(threadId: string, runsOn: RunsOn): Promise<void>;
}

/** What waits on the person (questions, approvals, secret requests): the request bar and the attention card. Its own interface because the server serves it from its own operations. */
export interface RequestsClient {
  /** Open requests across the space, newest first. */
  requests(): Promise<Request[]>;
  /** Subscribes to changes of the open requests. */
  watchRequests(cb: (requests: Request[]) => void): () => void;
  /** Answers a request. For a secret the value leaves the client here and is not kept. */
  answer(requestId: string, answer: Answer): Promise<void>;
  /** Where a provider sign-in stands: `pending`, `done`, `failed`, or `gone` (unknown or expired; the server answers not_found). Absent when the server has no provider sign-in. */
  signInStatus?(id: string): Promise<"pending" | "done" | "failed" | "gone">;
  /** The person started typing in a thread: the server may offer a sign-in the provider will need (it shows up in the open requests). Called once per focus; absent when there is no provider sign-in. */
  signInTyping?(threadId: string): Promise<void>;
}

/** The right panel's tabs (Changes, Run). */
export interface PanelsClient {
  /** The Changes tab. */
  changes(threadId: string): Promise<Change[]>;
  /** The Run tab. */
  run(threadId: string): Promise<RunInfo>;
}

/** The Threads app's whole data surface: the threads themselves, plus the three smaller surfaces above (each is what a server serves from its own operations). */
export interface ThreadsClient extends ChoicesClient, RequestsClient, PanelsClient {
  /** All threads the person can see in the current space, children included (`parentId`). */
  listThreads(): Promise<ThreadSummary[]>;
  /** One thread as it is now, with its real message count (the list does not know it); undefined when it does not exist or cannot be seen. */
  getThread(threadId: string): Promise<ThreadSummary | undefined>;
  /** Starts an empty thread (optionally under a parent). */
  createThread(title: string, options?: { provider?: ThreadSummary["provider"]; parentId?: string }): Promise<ThreadSummary>;
  /** Deletes a stopped thread: its file goes to the space's Trash. */
  deleteThread(threadId: string): Promise<void>;
  /** Subscribes to changes of the thread list (state, new threads). Returns the unsubscribe. */
  watchThreads(cb: (threads: ThreadSummary[]) => void): () => void;
  /** Messages `from` (inclusive) to `to` (exclusive), oldest first. At most a few hundred per call. */
  messages(threadId: string, from: number, to: number): Promise<Message[]>;
  /** Messages of the current space's threads that contain `query` (case-insensitive), newest first, at most `limit` (100 by default). Absent when the server has no message search yet (the backend lane's `threads.search`; the UI says so). */
  searchMessages?(query: string, limit?: number): Promise<MessageSearch>;
  /** Where a message is in its thread (its index among the thread's messages), so a list can jump to it without loading what is between; undefined when it is not there. */
  messageIndex?(threadId: string, messageId: string): Promise<number | undefined>;
  /** The picker and search lookups when the server has them (search.query, people.lookup, skills.list, threads.lookup, items.lookup): the composer's `@` and `/` and message search use these instead of the fixtures. */
  lookups?: Lookups;
  /** Snoozes a thread until `until` (milliseconds since the epoch) or, with null, brings it back now. Absent when the server has no snooze yet; then no snooze control shows. */
  snooze?(threadId: string, until: number | null): Promise<void>;
  /** Edits a sent message of the person's: the edit becomes another version of the thread from that message on (the old tail is kept) and the agent answers it. Absent when the server has no edit yet. */
  editMessage?(threadId: string, messageId: string, text: string, requestId: string): Promise<void>;
  /** Shows another version of an edited message and what follows it (1-based `version`); per thread, so every viewer sees the same. */
  setVersion?(threadId: string, messageId: string, version: number): Promise<void>;
  /** Follows a thread while it is open. Returns the unsubscribe. */
  watch(threadId: string, events: ThreadEvents): () => void;
  /** Sends a message (steers a running turn). `requestId` makes a retry safe. */
  send(threadId: string, text: string, requestId: string, attachments?: Attachment[]): Promise<void>;
  /** Uploads one attached file for the next message, reporting progress (0 to 1). Rejects with the reason in plain words when this server cannot take it. */
  upload(threadId: string, file: File, progress: (fraction: number) => void): Promise<void>;
  /** Stops the running turn. */
  stop(threadId: string): Promise<void>;
  /** Tries the last turn again (after a failure) or continues it from its last saved step (after an interruption). */
  resume(threadId: string): Promise<void>;
  /** Renames a thread. */
  rename(threadId: string, title: string): Promise<void>;
  /** Archives a thread (and its finished children): it leaves the list and keeps its file. */
  archive(threadId: string): Promise<void>;
  /** Takes back the last archive of this thread (and the finished children that went with it). Resolves false if it cannot. */
  unarchive(threadId: string): Promise<boolean>;
}

/** The answer to a message search. */
export interface MessageSearch { hits: MessageHit[]; /** The index is still catching up, rebuilding or has reached its coverage limit: say so, the list may be incomplete. */ partial: boolean }
/** One row of a picker lookup: a thread or an item. */
export interface PickerHit { id: string; title: string; kind: string; space: string }
/** The lookups behind message search and the composer's pickers. Each is one request; none throws for "no match" (an empty list). */
export interface Lookups {
  /** Messages that match, newest first, and whether the index is still catching up (`partial`: the results may be incomplete). */
  searchMessages(query: string, limit?: number): Promise<MessageSearch>;
  /** People and groups of the current space whose display name starts with `prefix` (empty browses). */
  people(prefix: string): Promise<{ id: string; name: string; kind: string }[]>;
  /** The current space's skills whose name starts with `prefix`. */
  skills(prefix: string): Promise<{ name: string; description: string }[]>;
  /** Threads by title prefix across the spaces the person can read. */
  threads(prefix: string): Promise<PickerHit[]>;
  /** Items (notes and the like) by title prefix across the spaces the person can read. */
  items(prefix: string): Promise<PickerHit[]>;
}
