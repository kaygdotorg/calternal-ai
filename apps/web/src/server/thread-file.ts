/**
 * The saved thread's wire shapes and its Markdown, as `docs/threads.md` and `crates/server/src/threads` define them, turned into the Threads app's own types.
 * Pure functions, no network. What the server sends:
 *   - `threads.list`   `{ threads: [{ id, address, revision, turn_state, metadata }] }`; no file is read for it
 *   - `threads.get`, `.create`, `.send`, `.rename`, `.archive`, `.unarchive`, `.delete`, `.stop`   `{ thread: { id, space_id, address, revision, turn_state, metadata, markdown }, job?: JobView }`
 *     (`get` carries the durable job while a turn is not final, to follow its stream after a reload)
 *   - `metadata`: `{ title, provider, model|null, mode, mode_for, message_count, last_activity_at, created_at, archived, deleted, parent_thread?, spawned_by?, pending? }`
 *   - `turn_state`: `idle | queued | running | waiting | interrupted | failed`
 *   - `markdown`: a small frontmatter, then messages: `<!-- message:<uuid> role:person|agent time:<rfc3339> [turn:<job>] [actor:<..>] -->` text `<!-- /message -->`.
 * Message text is untrusted display text: nothing here reads access, roles or instructions from it.
 */
import type { Message, Permission, ProviderId, ThreadState, ThreadSummary } from "../apps/threads/model";

/** The thread metadata the server keeps (the fields this client reads). */
export interface WireMetadata {
  title: string;
  provider: string;
  mode: string;
  /** Null until a turn has named one. */
  model?: string | null;
  message_count?: number;
  last_activity_at?: string;
  created_at?: string;
  archived?: boolean;
  deleted?: boolean;
  parent_thread?: string;
  spawned_by?: string;
  pending?: { request_id?: string; created_ms?: number };
}
/** A saved thread as `threads.get` and the writes return it. */
export interface WireThread { id: string; space_id: string; address: string; revision: string; turn_state?: string; metadata: WireMetadata; markdown?: string; job?: WireJob }
/** One row of `threads.list`. */
export interface WireRow { id: string; address: string; revision: string; turn_state?: string; metadata: WireMetadata }
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

/** Milliseconds since the epoch of an RFC 3339 time, or `fallback`. */
const ms = (t: string | undefined, fallback: number): number => { const v = t ? Date.parse(t) : NaN; return Number.isFinite(v) ? v : fallback; };

/** A thread's summary from its metadata and turn state. What the server does not know (where it runs) and what this page chose for the next send (`known`: provider, model, mode, machine) is laid over it. */
export function summaryOf(id: string, spaceId: string, md: WireMetadata, turn: string | undefined, known: Partial<ThreadSummary> = {}): ThreadSummary {
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
  };
}

const OPEN = /<!-- message:([0-9a-fA-F-]{36}) role:(person|agent)([^>]*?) -->\n/g;
const CLOSE = "\n<!-- /message -->";

/** The messages of a thread's Markdown, oldest first, each with the time its marker records (an older file with none: the thread's creation time, the last one its update time). A message with no closing marker (still being written) runs to the next marker or the end. */
export function parseMessages(markdown: string, createdAt = 0, updatedAt = createdAt): Message[] {
  const body = markdown.startsWith("---\n") ? (markdown.split("\n---\n").slice(1).join("\n---\n")) : markdown;
  const at: { id: string; role: Message["role"]; from: number; start: number; time: number }[] = [];
  for (const m of body.matchAll(OPEN)) at.push({ id: m[1]!, role: m[2] === "person" ? "user" : "agent", from: m.index!, start: m.index! + m[0].length, time: ms(/\btime:(\S+)/.exec(m[3] ?? "")?.[1], NaN) });
  return at.map((m, i) => {
    const next = at[i + 1]?.from ?? body.length;
    let text = body.slice(m.start, next);
    const close = text.lastIndexOf(CLOSE);
    if (close >= 0) text = text.slice(0, close);
    return { id: m.id, role: m.role, at: Number.isFinite(m.time) ? m.time : i === at.length - 1 ? updatedAt : createdAt, blocks: [{ type: "text" as const, text: text.replace(/\n$/, "") }] };
  });
}

/** The `updated-at` of a thread's frontmatter, in milliseconds, or `fallback`. */
export function updatedAtOf(markdown: string, fallback: number): number {
  const m = /^updated-at: (\S+)$/m.exec(markdown.slice(0, 2048));
  return ms(m?.[1]?.replace(/^"|"$/g, ""), fallback);
}
