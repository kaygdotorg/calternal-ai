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
  /** The machine's display name, or "Automatic" while the server picks one for the turn; never an ID. */
  machine: string;
  /** `direct` runs on the machine with no isolation (the owner consented when enrolling it); `sandboxed` is the default. */
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
  | { kind: "interrupted"; machine?: string; resumable: boolean; why?: string; sinceMs?: number }
  | { kind: "thread-full" };

/** What the sidebar and the header need to know about a thread. A child thread has a `parentId`. */
export interface ThreadSummary {
  /** The thread's UUID, the same in the sidebar, the address and every operation. */
  id: string;
  /** The space the thread is in (a UUID). The sidebar lists only the current space's threads. */
  spaceId: string;
  /** The name the person gave it, or the one the server derived from the first message. */
  title: string;
  /** The parent thread's id for a child thread (it is listed under the parent, never alone); null at the top level. */
  parentId: string | null;
  /** Whether a turn is running, parked on a request of the person, failed or finished; `idle` for a thread with no turn in flight. */
  state: ThreadState;
  /** The workspace the sidebar groups by (a repository or folder name). */
  workspace: string;
  /** Milliseconds since the epoch of the last message. */
  updatedAt: number;
  /** Which agent program runs the thread's turns. */
  provider: ProviderId;
  /** The model id sent with the next turn; "default" lets the provider choose. */
  model: string;
  /** How much the agent may do without asking; applies from the next turn (a running turn keeps the one it started with). */
  permission: Permission;
  /** Where its turns run (see `RunsOn`). */
  runsOn: RunsOn;
  /** The reasoning level chosen for the next turn, one of the model's `levels`; absent means the provider's own default. */
  reasoning?: string;
  /** Total messages in the thread's file. The list never loads them all. */
  messageCount: number;
  /** What needs the person's attention before or after a turn (see `ThreadNotice`); absent when nothing does. */
  notice?: ThreadNotice;
}

/** A provider and the models it offers. Switching is a composer control and never creates a thread or a visible message. */
export interface ProviderOption {
  /** The provider's id, as the thread summary names it. */
  id: ProviderId;
  /** The provider's display name in the picker. */
  name: string;
  /** The models the composer offers for this provider, in picker order. */
  models: { id: string; name: string; /** The reasoning levels this model accepts, lowest first; absent when it has no such setting (or the server does not list them yet). */ levels?: string[]; /** The provider's own default model: a chip showing it is not showing a choice the person made. */ default?: boolean }[];
  /** False when the server has no list of this provider's models yet: `models` is then only the default, and the picker says so instead of answering a search with nothing. */
  listed?: boolean;
}

/** What a tool step did, for its icon. */
export type StepKind = "read" | "edit" | "run" | "search" | "spawn";

/** One folded tool call. `output` can be very long; the UI virtualises it. */
export interface Step {
  /** Unique within the turn (the job id plus a running count), so a reload of the same turn keeps each step's identity. */
  id: string;
  /** Chooses the step's icon. */
  kind: StepKind;
  /** What the tool did in words (the provider's summary); never the raw command, which is `detail`. */
  title: string;
  /** The command or path. */
  detail?: string;
  /** The tool's printed result when one is known; the live stream does not carry it today, so it is usually absent. */
  output?: string;
  /** `running` until the tool finishes, then `ok` or `failed`. */
  status: "ok" | "failed" | "running";
  /** How long the tool ran in milliseconds, measured by this client from the start and end events it saw. */
  ms: number;
}

/** A file or picture the person attached to the message they sent (`file`). `thought` is the provider's reasoning, shown folded as "Thought for 4 s". `card` carries a generated-UI description (packages/genui) drawn by the client; `thread` links a child. */
export type Block =
  | { type: "text"; text: string }
  | { type: "steps"; steps: Step[]; ms: number }
  | { type: "thought"; text: string; ms: number }
  | { type: "file"; name: string; kind: "image" | "file"; size: number; url?: string }
  | { type: "card"; source: string }
  | { type: "thread"; threadId: string };

/** Who wrote an assistant message and how long the turn took, as its marker in the thread file records it: so a person scrolling back can tell which provider, model and reasoning level a turn used. */
export interface MessageBy {
  /** The provider's id. */
  provider?: ProviderId;
  /** The model id the turn ran with. */
  model?: string;
  /** The reasoning level the turn ran with ("low", "high", …), when the provider has such a setting. */
  reasoning?: string;
  /** How long the turn took, in milliseconds. */
  ms?: number;
}

/** One message: a block in the thread's file with a stable ID. */
export interface Message {
  /** The message's id from its marker in the thread file; stable across reloads, used to find a message again. */
  id: string;
  /** `user` is the person, `agent` the provider. */
  role: "user" | "agent";
  /** Milliseconds since the epoch the message was written. */
  at: number;
  /** The message's pieces in order: reasoning, folded steps, text, a generated card, a child thread link. */
  blocks: Block[];
  /** For an assistant message: which provider, model and reasoning level wrote it and how long it took; absent for an older message that does not record it. */
  by?: MessageBy;
}

/** A question from an agent: a form described in the generated-UI language, drawn by the shell. It never expires. */
export interface QuestionRequest {
  /** The request's id on the server; the answer is addressed to it. */
  id: string;
  /** Discriminates the `Request` union. */
  kind: "question";
  /** The thread whose turn is parked on this request; empty when the server names none. */
  threadId: string;
  /** The form's title, shown above its fields. */
  title: string;
  /** The generated-UI (GenUI v2) source of the form, rendered by the shell. */
  form: string;
}

/** A request to approve one action. Drawn by the shell, never by generated UI; an agent cannot answer it. It never expires. */
export interface ApprovalRequest {
  /** The request's id on the server; the decision is addressed to it. */
  id: string;
  /** Discriminates the `Request` union. */
  kind: "approval";
  /** The thread whose turn is parked on this request; empty when the server names none. */
  threadId: string;
  /** A fixed heading ("Approval needed"); the bar leads with `action`, not this. */
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
  /** The request's id on the server; the value is provided against it. */
  id: string;
  /** Discriminates the `Request` union. */
  kind: "secret";
  /** The thread whose turn raised it; empty when the server names none. */
  threadId: string;
  /** The manifest name, for example `GITHUB_TOKEN`. */
  name: string;
  /** The human label for the prompt, for example "Forgejo access token". */
  label: string;
  /** Why the agent needs it, in the agent's words (shown, not trusted). */
  why: string;
  /** `personal` is stored for the account, `space` for the space named in `spaceId`. */
  scope: "personal" | "space";
  /** The space it belongs to (a UUID) when `scope` is `space`: the value is stored there. */
  spaceId?: string;
  /** A required secret offers "Not now" and an optional one "Skip" (the bar's wording); the server's request carries no such flag yet, so the adapter marks every secret required. */
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
  /** The file's path relative to the workspace root. */
  path: string;
  /** Whether the turn created, changed or removed the file. */
  status: "added" | "modified" | "deleted";
  /** Lines added; 0 for a deleted or binary file. */
  added: number;
  /** Lines removed; 0 for an added or binary file. */
  removed: number;
}

/** The Run tab: where the thread runs and what it has used. */
export interface RunInfo {
  /** The permission mode in words, as the Run tab shows it. */
  permission: string;
  /** The machine and mode in words ("dev-machine, sandboxed"). */
  runsOn: string;
  /** The workspace folder or repository name; empty when the server has none. */
  workspace: string;
  /** The git branch of the workspace; empty outside a repository. */
  branch: string;
  /** The provider's display name. */
  provider: string;
  /** The model name in use. */
  model: string;
  /** Usage so far, already formatted ("12k tokens"); empty when not known. */
  tokens: string;
  /** The thread state in words. */
  state: string;
}

/** Something being attached in the composer, before it is sent: a picture or a file, with how far its upload has got. */
export interface Attachment {
  /** Unique in this composer. */
  id: string;
  kind: "image" | "file";
  name: string;
  /** Bytes. */
  size: number;
  /** A local address of the picture, for its thumbnail (revoked when it is removed or sent). */
  url?: string;
  /** 0 to 1 while `status` is "uploading". */
  progress: number;
  /** `failed` carries `error`, in plain words; the person can remove it. */
  status: "uploading" | "ready" | "failed";
  error?: string;
}
