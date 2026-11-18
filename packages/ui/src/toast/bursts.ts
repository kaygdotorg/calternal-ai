// Which events share a toast, and what an Undo really undid. Plain data, tested without a browser.
//
// A burst is a run of the same kind of event close together: delete twelve blocks and it's one
// toast, "Block deleted ×12", not twelve. Events only share a toast when they are the same
// operation on the same thing, with the same severity and the same kind of follow-up. That's never
// decided by the words: two different notes' deletions say the same thing but stay apart.
// A burst ends when its toast goes away, when its Undo starts, or after a quiet gap: the next
// event then starts again at 1.

/** `result` (something done, maybe undoable), `error` (something failed, maybe retryable) or `loading` (something being waited for). */
export type Severity = "result" | "error" | "loading";

/** What one event can do if you take it back. Returns true if it was undone. */
export type UndoToken = () => boolean | Promise<boolean>;

/** One thing that happened and may need a toast: what (`op`, `context`), how it went (`severity`), the words, and the follow-up (a real `undo`, or an `action` like Retry). */
export type ToastEvent = {
  /** The operation, e.g. "block.delete". */
  op: string;
  /** What it happened to, e.g. the note's id. Different things never share a toast. */
  context?: string;
  severity: Severity;
  /** What happened, once: "Block deleted". A burst shows it with its count: "Block deleted ×12". */
  message: string;
  /** The real undo for this one event. Only events that can all be undone share an Undo. */
  undo?: UndoToken;
  /** An Undo for many things at once ("3 notes"): no later toast may replace it while the Undo works. */
  bulk?: boolean;
  /** Any other follow-up (e.g. Retry). */
  action?: { label: string; run: () => void };
};

/** Events that share an identity (`burstKey`) and arrived close together, shown as one toast with a count ("Block deleted ×12") and one Undo that runs every token, newest first. */
export type Burst = {
  id: string;
  key: string;
  count: number;
  /** One token per event, oldest first; Undo runs them newest first. */
  tokens: UndoToken[];
  last: number;
  event: ToastEvent;
};

/** The identity events must share to join a burst. */
export function burstKey(e: ToastEvent): string {
  const follow = e.undo ? "undo" : e.action ? `action:${e.action.label}` : "none";
  return [e.op, e.context ?? "", e.severity, follow].join("|");
}

/** The grouping rule for toasts: events with the same key within the burst window join one burst; a bulk Undo blocks later toasts until it finishes. Pure state, no timers of its own; the caller supplies the clock. */
export class Bursts {
  private open = new Map<string, Burst>(); // by key: bursts that can still grow
  private seq = 0;

  constructor(public window = 4000) {}

  /** Adds an event: joins the open burst it belongs to, or starts a new one. */
  add(e: ToastEvent, now: number): Burst {
    const key = burstKey(e);
    const b = this.open.get(key);
    // Loading toasts each stand for their own wait; they never merge.
    if (b && e.severity !== "loading" && now - b.last <= this.window) {
      b.count++;
      b.last = now;
      b.event = e;
      if (e.undo) b.tokens.push(e.undo);
      return b;
    }
    const fresh: Burst = { id: `t${++this.seq}`, key, count: 1, tokens: e.undo ? [e.undo] : [], last: now, event: e };
    this.open.set(key, fresh);
    return fresh;
  }

  /** Stops a burst from growing (its toast went away, or its Undo started). */
  close(id: string) {
    for (const [k, b] of this.open) if (b.id === id) this.open.delete(k);
  }

  /** Is this burst still open to new events? */
  isOpen(id: string): boolean {
    for (const b of this.open.values()) if (b.id === id) return true;
    return false;
  }
}

/** What running a burst's Undo did: how many of its tokens undid, how many there were, and the tokens left (those that failed) so a retry can run them again. */
export type UndoOutcome = { undone: number; total: number; left: UndoToken[] };

/**
 * Runs a burst's undo tokens, newest first, stopping at the first that fails (later ones may
 * depend on it). Says exactly how many were undone, and which are left to try again.
 */
export async function undoAll(tokens: UndoToken[]): Promise<UndoOutcome> {
  const total = tokens.length;
  const todo = [...tokens];
  let undone = 0;
  while (todo.length) {
    const t = todo[todo.length - 1]!;
    let ok = false;
    try {
      ok = await t();
    } catch {
      ok = false;
    }
    if (!ok) break;
    todo.pop();
    undone++;
  }
  return { undone, total, left: todo };
}
