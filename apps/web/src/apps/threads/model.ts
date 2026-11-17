/**
 * The Threads app's data shapes, as the future operations will return them (docs/orchestrator-proposal.md). Every type here is
 * fixture-backed for now (fixtures.ts); the real client implements `ThreadsClient` (client.ts) and nothing else changes.
 */

/** A thread's life: `waiting` means a request needs the person, `failed` and `done` keep it visible until it archives. */
export type ThreadState = "idle" | "running" | "waiting" | "failed" | "done";

/**
 * How much the agent may do without asking, chosen per thread. A different choice from where it runs (`RunsOn`); the composer and the
 * thread details show them as two separate chips.
 */
export type Permission = "least" | "approve" | "full";

/** Where the thread runs: a machine, and whether in a sandbox or directly on a folder of the machine. */
export interface RunsOn {
  machine: string;
  mode: "sandboxed" | "direct";
}

/** Which official agent program runs the thread. */
export type ProviderId = "claude" | "codex";

/**
 * Something about a thread that needs the person's attention and has a next step: why a message cannot run yet, or what happened to the last turn. Drawn by `ThreadNotices` in plain words.
 * The server's `turn_reason` (docs/threads.md, turn readiness) becomes one of these (`reasonNotice`, server/thread-file.ts): `retrying` carries the automatic retry rule (provider backoff: 5 tries after 30 s, 1, 2, 4 and 8 minutes; API availability: a check every 3 hours; subscription: wait for the limit to reset); `interrupted` is a machine that went silent (nothing moves by itself; Resume continues it when it is resumable).
 */
export type ThreadNotice =
  | { kind: "no-machine" }
  | { kind: "waiting-capacity"; queuePosition?: number }
  | { kind: "machine-offline"; machine?: string; queuePosition?: number; sinceMs?: number }
  | { kind: "provider-signin"; machine?: string; provider: string; command?: string }
  | { kind: "failed"; failure: "transient" | "usage_limit" | "permanent"; reason?: string; resetAtMs?: number }
  | { kind: "retrying"; rule: "provider_backoff" | "api_availability" | "subscription_reset"; reason?: string; attempt?: number; of?: number; nextAt?: number }
  | { kind: "interrupted"; machine?: string; resumable: boolean; why?: string; sinceMs?: number };

/** What the sidebar and the header need to know about a thread. A child thread has a `parentId`. */
export interface ThreadSummary {
  id: string;
  /** The space the thread is in (a UUID). The sidebar lists only the current space's threads. */
  spaceId: string;
  title: string;
  parentId: string | null;
  state: ThreadState;
  /** The workspace the sidebar groups by (a repository or folder name). */
  workspace: string;
  /** Milliseconds since the epoch of the last message. */
  updatedAt: number;
  provider: ProviderId;
  model: string;
  permission: Permission;
  runsOn: RunsOn;
  /** Total messages in the thread's file. The list never loads them all. */
  messageCount: number;
  /** What needs the person's attention before or after a turn (see `ThreadNotice`); absent when nothing does. */
  notice?: ThreadNotice;
}

/** A provider and the models it offers. Switching is a composer control and never creates a thread or a visible message. */
export interface ProviderOption {
  id: ProviderId;
  name: string;
  models: { id: string; name: string }[];
}

/** What a tool step did, for its icon. */
export type StepKind = "read" | "edit" | "run" | "search" | "spawn";

/** One folded tool call. `output` can be very long; the UI virtualises it. */
export interface Step {
  id: string;
  kind: StepKind;
  title: string;
  /** The command or path. */
  detail?: string;
  output?: string;
  status: "ok" | "failed" | "running";
  ms: number;
}

/** A piece of a message. `card` carries a generated-UI description (packages/genui) drawn by the client; `thread` links a child. */
export type Block =
  | { type: "text"; text: string }
  | { type: "steps"; steps: Step[]; ms: number }
  | { type: "card"; source: string }
  | { type: "thread"; threadId: string };

/** One message: a block in the thread's file with a stable ID. */
export interface Message {
  id: string;
  role: "user" | "agent";
  at: number;
  blocks: Block[];
}

/** A question from an agent: a form described in the generated-UI language, drawn by the shell. It never expires. */
export interface QuestionRequest {
  id: string;
  kind: "question";
  threadId: string;
  title: string;
  form: string;
}

/** A request to approve one action. Drawn by the shell, never by generated UI; an agent cannot answer it. It never expires. */
export interface ApprovalRequest {
  id: string;
  kind: "approval";
  threadId: string;
  title: string;
  /** What the agent wants to do, in a sentence. */
  action: string;
  /** The exact command or change. */
  command: string;
  /** How long the person may allow it for. The server says: an irreversible or access-widening action is single-use (`once` only); the rest may also last for the thread. */
  lifetimes: ("once" | "thread")[];
}

/** A request for a secret. Drawn by the shell; the value goes to the server and the agent only ever gets a handle. Expires after 15 minutes. */
export interface SecretRequest {
  id: string;
  kind: "secret";
  threadId: string;
  /** The manifest name, for example `GITHUB_TOKEN`. */
  name: string;
  label: string;
  why: string;
  scope: "personal" | "space";
  /** The space it belongs to (a UUID) when `scope` is `space`: the value is stored there. */
  spaceId?: string;
  required: boolean;
  /** Milliseconds since the epoch. */
  expiresAt: number;
}

/** Anything that waits on the person. */
export type Request = QuestionRequest | ApprovalRequest | SecretRequest;

/** What the person answered. Secrets carry the value; the client sends it and drops it. */
export type Answer =
  | { kind: "question"; values: Record<string, unknown> }
  | { kind: "approval"; decision: "once" | "thread" | "deny" }
  | { kind: "secret"; value: string }
  | { kind: "secret-skip" };

/** One file a turn changed. */
export interface Change {
  path: string;
  status: "added" | "modified" | "deleted";
  added: number;
  removed: number;
}

/** The Run tab: where the thread runs and what it has used. */
export interface RunInfo {
  permission: string;
  runsOn: string;
  workspace: string;
  branch: string;
  provider: string;
  model: string;
  tokens: string;
  state: string;
}
