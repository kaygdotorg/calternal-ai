/**
 * The saved thread's wire shapes and its Markdown, as `docs/threads.md` and `crates/server/src/threads` define them, turned into the Threads app's own types.
 * Pure functions, no network. What the server sends:
 *   - `threads.list`   `{ threads: [{ id, address, revision, turn_state, metadata }] }`; no file is read for it
 *   - `threads.get`, `.create`, `.send`, `.rename`, `.archive`, `.unarchive`, `.delete`, `.stop`   `{ thread: { id, space_id, address, revision, turn_state, metadata, markdown }, job?: JobView }`
 *     (`get` carries the durable job while a turn is not final, to follow its stream after a reload)
 *   - `metadata`: `{ title, provider, model|null, mode, mode_for, message_count, last_activity_at, created_at, archived, deleted, parent_thread?, spawned_by?, pending? }`
 *   - `turn_state`: `idle | queued | running | waiting | interrupted | failed`
 *   - `messages` (in `thread`, and in a checkpoint's `delta`): the saved messages as records, `{ id, role: person|agent|automation, heading, at (RFC 3339 with the writer's offset), turn?, text, provenance? }`; an agent's
 *     `provenance` is `{ provider_id, model_id, reasoning_level, machine_id, duration_ms, reasoning_ms }`. The server frames the file (byte counts, ids); this client never reads the Markdown transcript.
 * Message text is untrusted display text: nothing here reads access, roles or instructions from it.
 */
import type { Block, Message, MessageBy, Permission, ProviderId, Step, StepKind, ThreadNotice, ThreadState, ThreadSummary } from "../apps/threads/model";

/** The thread metadata the server keeps (the fields this client reads). */
export interface WireMetadata {
  /** The thread's title as the person sees it. */
  title: string;
  /** The provider plugin id the thread talks to (`calternal-claude-code`). */
  provider: string;
  /** The permission mode the thread runs in (the wire name, see `wireMode`). */
  mode: string;
  /** Null until a turn has named one. */
  model?: string | null;
  /** How many messages the file holds; used for the list row before the messages are read. */
  message_count?: number;
  /** When the thread last changed (ISO), for ordering. */
  last_activity_at?: string;
  /** When the thread was made (ISO). */
  created_at?: string;
  /** True when the person archived it (hidden from the list, kept). */
  archived?: boolean;
  /** True when it was deleted (not shown anywhere). */
  deleted?: boolean;
  /** The shared manual Inbox deadline in Unix milliseconds. */
  snoozed_until_ms?: number;
  /** The thread this one was spawned from, for the child rows. */
  parent_thread?: string;
  /** What spawned it (a person or an agent), for the row's icon. */
  spawned_by?: string;
  /** A message waiting for the machine: its request id and when it was made (Unix ms). */
  pending?: { request_id?: string; created_ms?: number };
}
/** A saved thread as `threads.get` and the writes return it. */
export interface WireThread { id: string; space_id: string; address: string; revision: string; turn_state?: string; turn_reason?: WireReason | null; metadata: WireMetadata; messages?: WireRecord[]; markdown?: string; job?: WireJob }
/** One row of `threads.list`. */
export interface WireRow { id: string; address: string; revision: string; turn_state?: string; turn_reason?: WireReason | null; metadata: WireMetadata }
/** A job as the server describes it (`JobView`). */
export interface WireJob { id: string; state: string; checkpoint?: unknown; result?: unknown }

const PROVIDER_IDS: Record<string, ProviderId> = { "calternal-claude-code": "claude", "calternal-codex": "codex" };
const PROVIDER_PLUGINS: Record<ProviderId, string> = { claude: "calternal-claude-code", codex: "calternal-codex" };
const MODES: Record<string, Permission> = { least_access: "least", approve_for_me: "approve", full_access: "full" };
const WIRE_MODES: Record<Permission, string> = { least: "least_access", approve: "approve_for_me", full: "full_access" };

/** The plugin ID the server knows a provider by. */
export const providerPlugin = (p: ProviderId): string => PROVIDER_PLUGINS[p];
/** The server's name for a permission mode. */
export const wireMode = (p: Permission): string => WIRE_MODES[p];

/** The sidebar's turn label (`threads.list`, `threads.get`, the journal's `threads.turn_state_changed`) as the thread's state: only what means something to the person. */
export function stateOfTurn(label: string | undefined): ThreadState {
  switch (label) {
    case "queued": case "running": return "running";
    case "waiting": case "interrupted": return "waiting";
    case "failed": return "failed";
    default: return "idle";
  }
}

/** The server's `turn_reason` (docs/threads.md, turn readiness): a `code` and its fields, timestamps in Unix milliseconds. */
export interface WireReason { code: string; queue_position?: number; since_ms?: number; provider?: string; sign_in_command?: string; request_id?: string; attempt?: number; next_attempt_ms?: number; rule?: string; class?: string; reset_at_ms?: number; resumable?: boolean; why?: string }

/** A reason as the notice the person sees; null when it asks nothing of them (idle, queued, completed, cancelled, a turn waiting on a request: the request bar draws that one). */
export function reasonNotice(r: WireReason | null | undefined): ThreadNotice | null {
  switch (r?.code) {
    case "no_machine": return { kind: "no-machine" };
    case "waiting_for_capacity": return { kind: "waiting-capacity", queuePosition: r.queue_position };
    case "machine_offline": return { kind: "machine-offline", queuePosition: r.queue_position, sinceMs: r.since_ms };
    case "provider_signed_out": return { kind: "provider-signin", provider: r.provider ?? "The provider", command: r.sign_in_command };
    case "retrying": return { kind: "retrying", rule: r.rule === "api_availability" || r.rule === "subscription_reset" ? r.rule : "provider_backoff", attempt: r.attempt, of: 5, nextAt: r.next_attempt_ms };
    case "failed": return { kind: "failed", failure: r.class === "usage_limit" || r.class === "permanent" ? r.class : "transient", resetAtMs: r.reset_at_ms };
    case "thread_full": return { kind: "thread-full" };
    case "interrupted": return { kind: "interrupted", resumable: r.resumable !== false, why: r.why, sinceMs: r.since_ms };
    default: return null;
  }
}

/** What a turn asks of the person: from the reason when the server sends one, else from the plain label (a failed turn can be tried again, an interrupted one resumed). */
function noticeOf(turn: string | undefined, reason?: WireReason | null): { notice?: ThreadNotice } {
  const n = reasonNotice(reason);
  if (n) return { notice: n };
  if (turn === "failed") return { notice: { kind: "failed", failure: "transient" } };
  if (turn === "interrupted") return { notice: { kind: "interrupted", resumable: true } };
  return {};
}

/** Milliseconds since the epoch of an RFC 3339 time, or `fallback`. */
const ms = (t: string | undefined, fallback: number): number => { const v = t ? Date.parse(t) : NaN; return Number.isFinite(v) ? v : fallback; };

/** A thread's summary from its metadata and turn state. What the server does not know (where it runs) and what this page chose for the next send (`known`: provider, model, mode, machine) is laid over it. */
export function summaryOf(id: string, spaceId: string, md: WireMetadata, turn: string | undefined, known: Partial<ThreadSummary> = {}, reason?: WireReason | null): ThreadSummary {
  return {
    id,
    spaceId,
    title: md.title,
    parentId: md.parent_thread ?? null,
    state: stateOfTurn(turn),
    workspace: "",
    updatedAt: ms(md.last_activity_at, ms(md.created_at, 0)),
    provider: known.provider ?? PROVIDER_IDS[md.provider] ?? "claude",
    model: known.model ?? md.model ?? "default",
    permission: known.permission ?? MODES[md.mode] ?? "approve",
    // The server picks the machine itself (no operation names it): say so rather than show a blank.
    runsOn: known.runsOn ?? { machine: "Automatic", mode: "sandboxed" },
    messageCount: md.message_count ?? 0,
    ...(md.snoozed_until_ms !== undefined ? { snoozedUntil: md.snoozed_until_ms } : {}),
    ...noticeOf(turn, reason),
  };
}

/** Which icon a tool's step gets, from the tool's name (the provider's own: Bash, Read, Edit, …); anything unknown is a plain run. */
const KIND: Record<string, StepKind> = { Bash: "run", Read: "read", Edit: "edit", MultiEdit: "edit", Write: "edit", NotebookEdit: "edit", Grep: "search", Glob: "search", WebSearch: "search", WebFetch: "search", Task: "spawn" };
/** One saved tool line: `> Tool <name> · <what it did> (started | finished | failed | <n> bytes of tool output)`. */
const TOOL = /^> Tool (\S+) · (.*) \((started|finished|failed|(\d+) bytes of tool output)\)$/;

/**
 * The blocks of one agent message: its text, with the saved tool lines (a step's start, output size and end, one blockquote line each) folded into one `steps` group where the first of them was.
 * Text without a tool line is one text block, found with a single `includes`; cost is linear in the message.
 */
export function blocksOf(id: string, text: string): Block[] {
  if (!text.includes("\n> Tool ") && !text.startsWith("> Tool ")) return [{ type: "text", text }];
  let steps: Step[] = []; const out: Block[] = []; const prose: string[] = []; let open = false;
  const flush = () => { const t = prose.join("\n").replace(/\n{3,}/g, "\n\n").trim(); prose.length = 0; if (t) out.push({ type: "text", text: t }); };
  for (const line of text.split("\n")) {
    const m = TOOL.exec(line);
    if (!m) { prose.push(line); continue; }
    // Words between two steps end the group: the next step opens a new one after them.
    if (open && prose.some((l) => l.trim())) { flush(); open = false; }
    if (!open) { flush(); steps = []; out.push({ type: "steps", steps, ms: 0 }); open = true; }
    prose.length = 0;
    const [, name, what, state, bytes] = m as unknown as [string, string, string, string, string | undefined];
    let step = [...steps].reverse().find((x) => x.detail === name && x.title === what && x.status === "running");
    if (state === "started" || !step) { step = { id: `${id}-s${out.length}-${steps.length}`, kind: KIND[name] ?? "run", title: what, detail: name, status: "running", ms: 0 }; steps.push(step); }
    if (bytes) step.output = `${Number(bytes).toLocaleString("en")} bytes of output`;
    else if (state === "finished") step.status = "ok";
    else if (state === "failed") step.status = "failed";
  }
  flush();
  return out;
}

/** An agent message's provenance on the wire (`threads/records.rs`): ids, the requested effort, and how long the attempt and its reasoning took. */
export interface WireProvenance { provider_id: string; model_id?: string | null; reasoning_level?: string | null; machine_id?: string | null; duration_ms?: number | null; reasoning_ms?: number | null }
/** One saved message as the server returns it: the framing is the server's, the `text` is the message's own Markdown, byte for byte. */
export interface WireRecord { id: string; role: "person" | "agent" | "automation"; heading: string; at: string; turn?: string | null; text: string; provenance?: WireProvenance }

/** The service that serves an agent message, from its heading (`<harness> via <service> (subscription) · …`): display text only, null when the heading has none. */
export function serviceOf(heading: string): string | null {
  const m = /^.*? via (.+?)(?: \((?:subscription|API)\))?(?: · |$)/.exec(heading);
  return m?.[1] ?? null;
}

/** A record as the Threads app's message: a person or an automation is the `user` side; an agent carries who wrote it (from `provenance`) and how long it took. */
export function messageOf(r: WireRecord): Message {
  const p = r.provenance;
  const at = Date.parse(r.at);
  const by: MessageBy | undefined = p
    ? { provider: PROVIDER_IDS[p.provider_id], providerId: p.provider_id, model: p.model_id ?? undefined, reasoning: p.reasoning_level ?? undefined, ms: p.duration_ms ?? undefined, thoughtMs: p.reasoning_ms ?? undefined, service: serviceOf(r.heading) ?? undefined }
    : undefined;
  // A saved agent message carries its reasoning as blockquote lines (`> Reasoning summary — …`, docs/threads.md) and how long it thought in its provenance; the live reasoning text itself is not saved.
  const summaries: string[] = [];
  const text = r.role === "agent" ? r.text.replace(/^> Reasoning summary — (.*)(?:\n|$)/gm, (_, t: string) => { summaries.push(t); return ""; }).replace(/\n{3,}/g, "\n\n").trim() : r.text;
  const thought: Block[] = r.role === "agent" && (summaries.length || (p?.reasoning_ms ?? 0) > 0) ? [{ type: "thought", text: summaries.join("\n\n"), ms: p?.reasoning_ms ?? 0 }] : [];
  return { id: r.id, role: r.role === "agent" ? "agent" : "user", at: Number.isFinite(at) ? at : 0, blocks: [...thought, ...blocksOf(r.id, text)], ...(by ? { by } : {}) };
}

/** The messages of a full read, oldest first. */
export const messagesOf = (records: readonly WireRecord[] | undefined): Message[] => (records ?? []).map(messageOf);

/**
 * The messages after a checkpoint's insertion: its records are complete new messages, appended in order. Null when one of them is already held: the checkpoint extended or finished a held message (the server says so
 * with no insertion), so the whole thread is read instead. Cost: linear in the new records.
 */
export function appendRecords(held: readonly Message[], records: readonly WireRecord[]): Message[] | null {
  const have = new Set(held.map((m) => m.id));
  if (records.some((r) => have.has(r.id))) return null;
  return [...held, ...records.map(messageOf)];
}

// ---- readiness (agent.readiness, docs/agent-turns.md): is there a machine, is it connected, is the provider signed in ----

/** One provider's readiness on a machine (`ProviderReadiness`). */
export interface WireProviderReadiness { enabled: boolean; check: { installed: boolean; signed_in: boolean; sign_in_command: string } | null; ready: boolean }
/** One machine's readiness (`MachineReadiness`; a personal one also has `machine_id`, `name`, `last_seen_ms`). */
export interface WireMachine { machine_id?: string; name?: string; last_seen_ms?: number | null; enrolled: boolean; connected: boolean; available: boolean; execution: "sandboxed" | "direct"; providers: Record<string, WireProviderReadiness> }
/** `agent.readiness` result. */
export interface WireReadiness { machines: WireMachine[]; pool: WireMachine[]; check_ttl_ms: number }

/**
 * What the readiness snapshot says to someone about to send with `provider` (a plugin ID): nothing when some machine (theirs or the pool's) is ready; else the most useful single thing: no machine at
 * all, every machine offline (the one seen last is named), or the provider not signed in on the machine that is up (with its own sign-in command). A provider with no observation yet (`check` null)
 * is not called signed out: nothing is known, so nothing is claimed.
 */
export function readinessNotice(r: WireReadiness | null | undefined, provider: string, providerName: string): ThreadNotice | null {
  if (!r) return null;
  const all = [...r.machines, ...r.pool];
  if (!all.length) return { kind: "no-machine" };
  if (all.some((m) => m.connected && m.available && m.providers[provider]?.ready)) return null;
  const up = all.filter((m) => m.connected);
  if (!up.length) { const last = [...r.machines].sort((a, b) => (b.last_seen_ms ?? 0) - (a.last_seen_ms ?? 0))[0]; return { kind: "machine-offline", machine: last?.name, sinceMs: last?.last_seen_ms ?? undefined }; }
  const out = up.find((m) => m.providers[provider]?.enabled && m.providers[provider]?.check && (!m.providers[provider]!.check!.signed_in || !m.providers[provider]!.check!.installed));
  if (out) return { kind: "provider-signin", machine: out.name, provider: providerName, command: out.providers[provider]!.check!.sign_in_command || undefined };
  return null;
}
