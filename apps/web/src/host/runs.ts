/**
 * Real agent run log decoder and session persistence for mid-run reloads.
 * Parses the host's `run_log` stdout NDJSON frames (`text_delta`, `done`, `exit`)
 * without naming any specific plugin or provider.
 */
import type { RunLogChunk } from "./types";
import type { RunUiState } from "./HostRecords.svelte";

/** Run state persisted across page reloads so an active run resumes from its chunk offset. */
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
  lastAnchorBefore?: string | null;
}

/** Result of feeding one `run_log` batch into [`RunLogParser`]. */
export interface ParsedRunBatch {
  textDelta: string;
  turnDone: boolean;
  exited: boolean;
  exitCode: number | null;
}

/** Stateful UTF-8 base64 + line-buffered NDJSON parser for `run_log` stdout chunks. */
export class RunLogParser {
  private readonly decoder = new TextDecoder("utf-8", { fatal: false });
  private pending = "";

  /** Feed one batch of `run_log` chunks and return any new reply text and lifecycle signals. */
  feed(chunks: RunLogChunk[]): ParsedRunBatch {
    let textDelta = "";
    let turnDone = false;
    let exited = false;
    let exitCode: number | null = null;

    for (const chunk of chunks) {
      if (chunk.event === "exit") {
        exited = true;
        exitCode = chunk.code ?? null;
        continue;
      }
      if (chunk.event !== "data" || chunk.stream !== "stdout") continue;
      let bytes: Uint8Array;
      try {
        const bin = atob(chunk.bytes);
        bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      } catch {
        continue;
      }
      this.pending += this.decoder.decode(bytes, { stream: true });
      const lines = this.pending.split("\n");
      this.pending = lines.pop() ?? "";
      for (const raw of lines) {
        const line = raw.trim();
        if (!line) continue;
        try {
          const parsed = JSON.parse(line) as {
            event?: { type?: string; text?: string; outcome?: string };
          };
          const ev = parsed.event;
          if (!ev) continue;
          if (ev.type === "text_delta" && typeof ev.text === "string") {
            textDelta += ev.text;
          } else if (ev.type === "done") {
            turnDone = true;
          }
        } catch {
          // Ignore non-JSON stdout lines.
        }
      }
    }

    return { textDelta, turnDone, exited, exitCode };
  }
}

const runStorageKey = (space: string, itemId: string) => `calternal.host.run.${space}.${itemId}`;
const openItemKey = (space: string) => `calternal.host.open.${space}`;

/** Load a persisted in-flight run for `itemId` from `sessionStorage` (survives page reload). */
export function loadPersistedRun(space: string, itemId: string): PersistedRun | null {
  try {
    const raw = sessionStorage.getItem(runStorageKey(space, itemId));
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedRun;
    return parsed && typeof parsed.id === "string" ? parsed : null;
  } catch {
    return null;
  }
}

/** Save an in-flight run snapshot to `sessionStorage` so reload resumes from `offset`. */
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
    return sessionStorage.getItem(openItemKey(space));
  } catch {
    return null;
  }
}
