// Ported from the existing app (origin/dev packages/ui/src/primitives/toast), logic and motion unchanged except where noted.
// Toasts (#42): this file decides what a toast says and groups bursts. Each toast is one
// renderer (ToastCapsule), updated in place by its id. An app with a bottom bar shows them in the
// bar (inbar.svelte.ts, #145): then the bar queues and times them. Screens without a bar use the
// small stack (stack.svelte.ts), which times and lists them.
//
// Calternal's toast rule (T1): a toast is for something you can act on (Undo, Retry) or the
// result of something you were waiting for. Never a plain "Saved". So `notify` asks for one of
// those, and says so in development if it doesn't get it.
import { SvelteMap } from "svelte/reactivity";
import { Bursts, undoAll, type ToastEvent, type UndoToken } from "./bursts";
import type { Presenter } from "./presenter";
import { stackPresenter, onStackEnd } from "./stack.svelte";

/** Where a toast is in its life: shown, loading, undoing, undone, partially undone or failed. Drives its text and whether its button is busy. */
export type Phase = "shown" | "loading" | "undoing" | "undone" | "partial" | "failed";

/** What one visible toast renders: its words, its count and phase, and its follow-up button (Undo, with tokens left, or another action). */
export type ToastView = {
  id: string;
  severity: ToastEvent["severity"];
  text: string;
  count: number;
  phase: Phase;
  /** The follow-up button, if any: Undo (with tokens left to undo) or another action. */
  action?: { label: string; run: () => void; busy?: boolean };
};

/** Every toast on screen, by id: what the renderer draws. */
export const views = new SvelteMap<string, ToastView>();

// A repeat joins the toast while it is still on screen (the longest a toast with Undo stays); once
// the toast has gone, its burst is closed and the next event starts again at 1.
const bursts = new Bursts(8000);
const tokensOf = new Map<string, UndoToken[]>();

const LIFE = { result: 5000, error: 9000, undo: 8000, settle: 1800 } as const;

// Where new toasts go, and where each one on screen lives (a toast stays where it first showed).
let presenter: Presenter = stackPresenter;
const where = new Map<string, Presenter>();

// A bulk toast ("3 notes") whose Undo still works is never replaced or dropped: the next toast waits behind it.
const waiting = new Set(["loading", "undoing"]);
const bulkIds = new Set<string>(); // toasts for a bulk action ("3 notes") with an Undo
const isUndoable = (id: string) => {
  const ph = views.get(id)?.phase;
  return bulkIds.has(id) && (ph === "shown" || ph === "partial" || ph === "failed") && !!tokensOf.get(id)?.length;
};
// Toasts waiting behind an Undo, in order (a toast not on screen yet has no place in `where`).
const pending: { id: string; duration: number }[] = [];

function show(id: string, duration: number) {
  if (!where.has(id)) {
    // A bulk Undo on screen keeps its place. The newcomer queues behind it; a toast already
    // queued is replaced by a newer one, but a queued bulk Undo is never dropped.
    if ([...where.keys()].some(isUndoable)) {
      const queued = pending.find((q) => q.id === id);
      if (queued) queued.duration = duration;
      else {
        if (!bulkIds.has(id))
          for (let i = pending.length - 1; i >= 0; i--)
            if (!bulkIds.has(pending[i].id)) {
              const [gone] = pending.splice(i, 1);
              bursts.close(gone.id);
              tokensOf.delete(gone.id);
              views.delete(gone.id);
            }
        pending.push({ id, duration });
      }
      return;
    }
    // One toast at a time, the newest: a different toast takes the place of the one on screen, in the
    // bar or in the stack (never drawn over it). A wait that is still running is left alone.
    for (const [other, o] of where) if (!waiting.has(views.get(other)?.phase ?? "")) o.dismiss(other);
  }
  const p = where.get(id) ?? presenter;
  where.set(id, p);
  p.show(id, duration);
}

function end(id: string) {
  bursts.close(id);
  tokensOf.delete(id);
  bulkIds.delete(id);
  views.delete(id);
  where.delete(id);
  const q = pending.findIndex((e) => e.id === id);
  if (q >= 0) pending.splice(q, 1); // a queued toast that was dismissed
  if (pending.length && ![...where.keys()].some(isUndoable)) {
    const next = pending.shift()!;
    show(next.id, next.duration);
  }
}
onStackEnd(end);

/**
 * Shows (or grows) a toast. Same operation on the same thing within a few seconds: the same toast,
 * counting up, its lifetime refreshed. Returns its id.
 */
export function notify(e: ToastEvent & { awaited?: boolean }): string {
  const safe = { ...e };
  if (import.meta.env?.DEV && safe.severity === "result" && !safe.undo && !safe.action && !safe.awaited)
    console.warn(`Toast "${safe.message}" has nothing to act on and nothing awaited: Calternal doesn't toast plain confirmations (T1).`);
  const b = bursts.add(safe, Date.now());
  if (e.undo) tokensOf.set(b.id, b.tokens);
  if (e.undo && e.bulk) bulkIds.add(b.id);
  views.set(b.id, {
    id: b.id,
    severity: safe.severity,
    text: safe.message,
    count: b.count,
    phase: safe.severity === "loading" ? "loading" : "shown",
    action: safe.undo ? { label: "Undo", run: () => undo(b.id, safe) } : safe.action,
  });
  show(b.id, safe.severity === "loading" ? Number.POSITIVE_INFINITY : safe.undo ? LIFE.undo : safe.severity === "error" ? LIFE.error : LIFE.result);
  return b.id;
}

/** Runs the newest toast's Undo, if one is showing. The keyboard's way to Undo (Mod+Z outside a text field): the toast's own button sits at the end of the page, a long way along a Tab walk. Returns whether there was one. */
export function undoLatest(): boolean {
  const all = [...views.values()].filter((v) => v.action?.label === "Undo" && v.phase === "shown" && !v.action.busy);
  const v = all[all.length - 1];
  if (!v?.action) return false;
  void v.action.run();
  return true;
}

/** Undo for a burst: honest about what happened, never claims more than it did. */
async function undo(id: string, e: ToastEvent) {
  const v = views.get(id);
  const tokens = tokensOf.get(id);
  if (!v || !tokens?.length || v.phase === "undoing") return; // no double Undo while one is running
  bursts.close(id); // new events start a new toast from here on
  views.set(id, { ...v, phase: "undoing", action: { ...v.action!, busy: true } });
  show(id, Number.POSITIVE_INFINITY); // it stays while undoing
  const out = await undoAll(tokens);
  if (!views.has(id)) return; // dismissed meanwhile: nothing to show
  if (out.undone === out.total) {
    views.set(id, { id, severity: "result", text: "Undone", count: 1, phase: "undone" });
    show(id, LIFE.settle);
    return;
  }
  // Some or none: say exactly that, and keep the rest ready to try again.
  tokensOf.set(id, out.left);
  const text = out.undone > 0 ? `Undid ${out.undone} of ${out.total}` : out.total === 1 ? "Couldn't undo that" : `Couldn't undo those ${out.total}`;
  views.set(id, {
    id,
    severity: "error",
    text,
    count: 1,
    phase: out.undone === 0 ? "failed" : "partial",
    action: { label: "Try again", run: () => undo(id, e) },
  });
  show(id, LIFE.error);
}

/**
 * A wait you'll want to know the end of: one toast that says it's working until the promise
 * settles, then turns into the result (or the error) in place.
 */
export function track<T>(
  promise: Promise<T>,
  m: { op: string; context?: string; loading: string; success: (v: T) => string; error: (err: unknown) => string; retry?: () => void },
): string {
  const id = notify({ op: m.op, context: m.context, severity: "loading", message: m.loading });
  promise.then(
    (v) => {
      if (!views.has(id)) return;
      views.set(id, { id, severity: "result", text: m.success(v), count: 1, phase: "shown" });
      show(id, LIFE.result);
    },
    (err) => {
      if (!views.has(id)) return;
      views.set(id, {
        id,
        severity: "error",
        text: m.error(err),
        count: 1,
        phase: "failed",
        action: m.retry ? { label: "Retry", run: () => (dismiss(id), m.retry!()) } : undefined,
      });
      show(id, LIFE.error);
    },
  );
  return id;
}

/** Removes a toast by id from whichever presenter shows it; an unknown id does nothing. */
export const dismiss = (id: string) => (where.get(id) ?? presenter).dismiss(id);
