// Ported from the existing app (origin/dev packages/ui/src/primitives/toast), logic and motion unchanged except where noted.
// The app-facing calls. Rule (T1): a toast only for something you can act on (Undo, Retry) or a result you waited for, never as a plain confirmation.
import { notify } from "./notify.svelte";
import type { UndoToken } from "./bursts";

/** A result with an optional follow-up action. */
export function toast(text: string, action?: { label: string; run: () => void }): void {
  notify({ op: `app:${text}`, severity: "result", message: text, action });
}

/**
 * A result that can be undone: `undo` takes back exactly this change and says if it could.
 * `bulk`: it covers many things ("3 notes"), so a later toast waits behind it until it is over.
 */
export function undoable(op: string, context: string, message: string, undo: UndoToken, bulk = false): void {
  notify({ op, context, severity: "result", message, undo, bulk });
}

/** Something failed; `retry` offers to try again. */
export function failed(message: string, retry?: () => void): void {
  notify({ op: `error:${message}`, severity: "error", message, action: retry ? { label: "Try again", run: retry } : undefined });
}
