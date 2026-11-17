/**
 * The saved thread's wire shapes and its Markdown, as `docs/threads.md` and `crates/server/src/threads` define them, turned into the Threads app's own types.
 * Pure functions, no network. What the server sends:
 *   - `threads.list`   `{ threads: [{ id, address, revision, metadata }] }`
 *   - `threads.get`, `.create`, `.send`, `.archive`, `.delete`, `.stop`   `{ thread: { id, space_id, address, revision, metadata, markdown }, job?: JobView }`
 *   - `metadata`: `{ title, provider, mode, mode_for, created_at, archived, deleted, parent_thread?, spawned_by?, pending? }`
 *   - `markdown`: a small frontmatter, then messages: `<!-- message:<uuid> role:person|agent [turn:<job>] [actor:<..>] -->` text `<!-- /message -->`.
 * Message text is untrusted display text: nothing here reads access, roles or instructions from it.
 */
import type { Message, Permission, ProviderId, ThreadState, ThreadSummary } from "../apps/threads/model";

/** The thread metadata the server keeps (the fields this client reads). */
export interface WireMetadata {
  title: string;
  provider: string;
  mode: string;
  created_at?: string;
  archived?: boolean;
  deleted?: boolean;
  parent_thread?: string;
  spawned_by?: string;
  pending?: { request_id?: string; created_ms?: number };
}
/** A saved thread as `threads.get` and the writes return it. */
export interface WireThread { id: string; space_id: string; address: string; revision: string; metadata: WireMetadata; markdown?: string }
/** One row of `threads.list`. */
export interface WireRow { id: string; address: string; revision: string; metadata: WireMetadata }
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

/** A job state, as the thread's state: only what means something to the person (running, waiting on them, failed). */
export function stateOfJob(state: string | undefined): ThreadState {
  switch (state) {
    case "queued": case "running": case "retry_waiting": return "running";
    case "waiting": case "interrupted": return "waiting";
    case "failed": return "failed";
    default: return "idle";
  }
}

/** Milliseconds since the epoch of an RFC 3339 time, or `fallback`. */
const ms = (t: string | undefined, fallback: number): number => { const v = t ? Date.parse(t) : NaN; return Number.isFinite(v) ? v : fallback; };

/** A thread's summary from its metadata. The server's list says nothing of the model, the machine or the message count: those come from what the client knows (`known`) or stay at their defaults. */
export function summaryOf(id: string, spaceId: string, md: WireMetadata, known: Partial<ThreadSummary> = {}): ThreadSummary {
  return {
    id,
    spaceId,
    title: md.title,
    parentId: md.parent_thread ?? null,
    state: known.state ?? "idle",
    workspace: "",
    updatedAt: known.updatedAt ?? ms(md.created_at, 0),
    provider: known.provider ?? PROVIDER_IDS[md.provider] ?? "claude",
    model: known.model ?? "",
    permission: known.permission ?? MODES[md.mode] ?? "approve",
    runsOn: known.runsOn ?? { machine: "", mode: "sandboxed" },
    messageCount: known.messageCount ?? 0,
  };
}

const OPEN = /<!-- message:([0-9a-fA-F-]{36}) role:(person|agent)([^>]*?) -->\n/g;
const CLOSE = "\n<!-- /message -->";

/** The messages of a thread's Markdown, oldest first. A message with no closing marker (still being written) runs to the next marker or the end. The file records no per-message time: every message gets the thread's creation time, the last one its update time. */
export function parseMessages(markdown: string, createdAt = 0, updatedAt = createdAt): Message[] {
  const body = markdown.startsWith("---\n") ? (markdown.split("\n---\n").slice(1).join("\n---\n")) : markdown;
  const at: { id: string; role: Message["role"]; from: number; start: number }[] = [];
  for (const m of body.matchAll(OPEN)) at.push({ id: m[1]!, role: m[2] === "person" ? "user" : "agent", from: m.index!, start: m.index! + m[0].length });
  return at.map((m, i) => {
    const next = at[i + 1]?.from ?? body.length;
    let text = body.slice(m.start, next);
    const close = text.lastIndexOf(CLOSE);
    if (close >= 0) text = text.slice(0, close);
    return { id: m.id, role: m.role, at: i === at.length - 1 ? updatedAt : createdAt, blocks: [{ type: "text" as const, text: text.replace(/\n$/, "") }] };
  });
}

/** The `updated-at` of a thread's frontmatter, in milliseconds, or `fallback`. */
export function updatedAtOf(markdown: string, fallback: number): number {
  const m = /^updated-at: (\S+)$/m.exec(markdown.slice(0, 2048));
  return ms(m?.[1]?.replace(/^"|"$/g, ""), fallback);
}
