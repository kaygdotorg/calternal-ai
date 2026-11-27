/**
 * Real agent run log decoder and session persistence for mid-run reloads.
 * Parses the host's `run_log` stdout NDJSON frames (`text_delta`, `done`, `exit`)
 * with bounded line buffering and line-safe checkpoints so multi-byte UTF-8
 * sequences split across chunks are never lost on reload.
 */
import type { RunLogChunk } from "./types";
import type { RunUiState } from "./HostRecords.svelte";

const MAX_PENDING_BYTES = 256 * 1024;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const RESUMABLE_STATUSES = new Set<RunUiState>(["starting", "streaming", "stopping"]);

/** Run state persisted across page reloads so an active run resumes from its last line-safe offset. */
export interface PersistedRun {
  id: string;
  itemId: string;
  label: string;
  slot: string;
  filler: string;
  fillerOp: string;
  offset: number;
  text: string;
  status: RunUiState;
  promptLink?: string;
}

/** Result of feeding one `run_log` batch into [`RunLogParser`]. */
export interface ParsedRunBatch {
  /** New reply text decoded from completed NDJSON lines in this batch. */
  textDelta: string;
  /** Last chunk offset at a completed line boundary (safe to persist and resume from). */
  safeOffset: number;
  /** True once a `done` event has been observed on a completed line. */
  turnDone: boolean;
  /** True once an `exit` chunk has been observed. */
  exited: boolean;
  exitCode: number | null;
}

/**
 * Stateful byte-buffered NDJSON parser for `run_log` stdout chunks.
 * - Buffers raw bytes (not decoded strings) so UTF-8 code points split across chunks decode intact.
 * - Caps any single unterminated line at `MAX_PENDING_BYTES` and discards through the next `\n`.
 * - Tracks `safeOffset` only at completed line boundaries so reload replays any partial trailing line.
 */
export class RunLogParser {
  private readonly decoder = new TextDecoder("utf-8", { fatal: false });
  private pending = new Uint8Array(0);
  private discarding = false;
  private safeOffset: number;

  constructor(initialSafeOffset = 0) {
    this.safeOffset = initialSafeOffset;
  }

  /** Feed one batch of `run_log` chunks and return decoded text plus the latest line-safe offset. */
  feed(chunks: RunLogChunk[]): ParsedRunBatch {
    let textDelta = "";
    let turnDone = false;
    let exited = false;
    let exitCode: number | null = null;

    for (const chunk of chunks) {
      if (chunk.event === "exit") {
        exited = true;
        exitCode = chunk.code ?? null;
        this.pending = new Uint8Array(0);
        this.discarding = false;
        this.safeOffset = chunk.offset;
        continue;
      }
      if (chunk.event !== "data" || chunk.stream !== "stdout") {
        if (this.pending.length === 0 && !this.discarding) {
          this.safeOffset = chunk.offset;
        }
        continue;
      }

      let bytes: Uint8Array;
      try {
        const bin = atob(chunk.bytes);
        bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      } catch {
        continue;
      }

      let start = 0;
      if (this.discarding) {
        const nl = bytes.indexOf(0x0a);
        if (nl < 0) continue;
        this.discarding = false;
        start = nl + 1;
      }

      while (start < bytes.length) {
        const nl = bytes.indexOf(0x0a, start);
        if (nl < 0) {
          const tail = bytes.subarray(start);
          if (this.pending.length + tail.length > MAX_PENDING_BYTES) {
            this.pending = new Uint8Array(0);
            this.discarding = true;
          } else {
            const merged = new Uint8Array(this.pending.length + tail.length);
            merged.set(this.pending, 0);
            merged.set(tail, this.pending.length);
            this.pending = merged;
          }
          break;
        }

        const slice = bytes.subarray(start, nl);
        let lineBytes: Uint8Array;
        if (this.pending.length > 0) {
          if (this.pending.length + slice.length > MAX_PENDING_BYTES) {
            this.pending = new Uint8Array(0);
            start = nl + 1;
            continue;
          }
          lineBytes = new Uint8Array(this.pending.length + slice.length);
          lineBytes.set(this.pending, 0);
          lineBytes.set(slice, this.pending.length);
          this.pending = new Uint8Array(0);
        } else {
          lineBytes = slice;
        }

        const line = this.decoder.decode(lineBytes).trim();
        if (line) {
          try {
            const parsed = JSON.parse(line) as {
              event?: { type?: string; text?: string; outcome?: string };
            };
            const ev = parsed.event;
            if (ev?.type === "text_delta" && typeof ev.text === "string") {
              textDelta += ev.text;
            } else if (ev?.type === "done") {
              turnDone = true;
            }
          } catch {
            // Ignore non-JSON stdout lines.
          }
        }
        start = nl + 1;
      }

      if (this.pending.length === 0 && !this.discarding) {
        this.safeOffset = chunk.offset;
      }
    }

    return { textDelta, safeOffset: this.safeOffset, turnDone, exited, exitCode };
  }
}

const runStorageKey = (space: string, itemId: string) => `calternal.host.run.${space}.${itemId}`;
const openItemKey = (space: string) => `calternal.host.open.${space}`;

/** Validate a stored [`PersistedRun`] against the expected item ID before resuming. */
export function validatePersistedRun(raw: unknown, expectedItemId: string): PersistedRun | null {
  if (!raw || typeof raw !== "object") return null;
  const r = raw as Record<string, unknown>;
  if (typeof r.id !== "string" || !UUID_RE.test(r.id)) return null;
  if (r.itemId !== expectedItemId) return null;
  if (typeof r.offset !== "number" || !Number.isInteger(r.offset) || r.offset < 0) return null;
  if (typeof r.text !== "string") return null;
  if (typeof r.label !== "string" || !r.label.trim()) return null;
  if (typeof r.slot !== "string" || typeof r.filler !== "string" || typeof r.fillerOp !== "string") return null;
  if (typeof r.status !== "string" || !RESUMABLE_STATUSES.has(r.status as RunUiState)) return null;
  if (r.promptLink !== undefined && typeof r.promptLink !== "string") return null;
  return {
    id: r.id,
    itemId: r.itemId,
    label: r.label,
    slot: r.slot,
    filler: r.filler,
    fillerOp: r.fillerOp,
    offset: r.offset,
    text: r.text,
    status: r.status as RunUiState,
    promptLink: r.promptLink as string | undefined,
  };
}

/** Load and validate a persisted in-flight run for `itemId` from `sessionStorage`. */
export function loadPersistedRun(space: string, itemId: string): PersistedRun | null {
  try {
    const raw = sessionStorage.getItem(runStorageKey(space, itemId));
    if (!raw) return null;
    const validated = validatePersistedRun(JSON.parse(raw), itemId);
    if (!validated) clearPersistedRun(space, itemId);
    return validated;
  } catch {
    clearPersistedRun(space, itemId);
    return null;
  }
}

/** Save a line-safe in-flight run checkpoint to `sessionStorage` so reload resumes cleanly. */
export function savePersistedRun(space: string, run: PersistedRun): void {
  try {
    sessionStorage.setItem(runStorageKey(space, run.itemId), JSON.stringify(run));
  } catch {
    // ignore storage errors
  }
}

/** Remove any persisted run state for `itemId`. */
export function clearPersistedRun(space: string, itemId: string): void {
  try {
    sessionStorage.removeItem(runStorageKey(space, itemId));
  } catch {
    // ignore storage errors
  }
}

/** Remember the currently open item ID across page reloads. */
export function saveLastOpenItem(space: string, itemId: string | null): void {
  try {
    if (itemId) sessionStorage.setItem(openItemKey(space), itemId);
    else sessionStorage.removeItem(openItemKey(space));
  } catch {
    // ignore storage errors
  }
}

/** Load the last open item ID for `space` from `sessionStorage`. */
export function loadLastOpenItem(space: string): string | null {
  try {
    const id = sessionStorage.getItem(openItemKey(space));
    return id && UUID_RE.test(id) ? id : null;
  } catch {
    return null;
  }
}
