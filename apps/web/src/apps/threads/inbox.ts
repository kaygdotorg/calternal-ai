/**
 * The Inbox of the Threads sidebar: only the threads that need the person (docs/threads-ui.md, "Inbox and All Threads"). A thread is in it while something waits on you: a question, an approval or a secret
 * request, a provider that needs you to sign in again, a turn that failed or one that was interrupted and needs a decision. Nothing else is: a running thread, a finished one, a thread with a notice that asks
 * nothing of you (queued, retrying by itself, waiting for a free machine) stays in All Threads only. Newest first.
 */
import type { Request, ThreadSummary } from "./model";

/** One thread in the Inbox and why, in the words the row shows. */
export interface InboxItem {
  thread: ThreadSummary;
  /** Short reasons, each once, in this order: Question, Approval, Secret, Sign in, Failed, Interrupted. */
  reasons: string[];
}

const ORDER = ["Question", "Approval", "Secret", "Sign in", "Failed", "Interrupted"];
const OF_REQUEST: Record<Request["kind"], string> = { question: "Question", approval: "Approval", secret: "Secret", signin: "Sign in" };

/** When `t` comes back if it is snoozed at `now`, else undefined: the person's snooze, or, with no action, a thread waiting for a subscription limit to reset (its retrying notice says when). */
export function snoozeEnd(t: ThreadSummary, now: number): { until: number; auto: boolean } | undefined {
  if (t.snoozedUntil !== undefined && t.snoozedUntil > now) return { until: t.snoozedUntil, auto: false };
  const n = t.notice;
  if (n?.kind === "retrying" && n.rule === "subscription_reset" && n.nextAt !== undefined && n.nextAt > now) return { until: n.nextAt, auto: true };
  return undefined;
}

/** A snoozed thread and when it returns. `auto` is a subscription limit waiting to reset (no manual snooze to undo). */
export interface SnoozedItem { thread: ThreadSummary; until: number; auto: boolean }

/** The snoozed threads, the soonest to return first. Cost: one pass. */
export function snoozedOf(threads: readonly ThreadSummary[], now: number): SnoozedItem[] {
  return threads.flatMap((thread) => { const s = snoozeEnd(thread, now); return s ? [{ thread, ...s }] : []; }).sort((a, b) => a.until - b.until);
}

/** The Inbox for these threads and their open requests: only what waits on the person and is not snoozed at `now`. Cost: one pass over each. */
export function inboxOf(threads: readonly ThreadSummary[], requests: readonly Request[], now = Date.now()): InboxItem[] {
  const by = new Map<string, Set<string>>();
  const add = (id: string, reason: string) => (by.get(id) ?? by.set(id, new Set()).get(id)!).add(reason);
  for (const r of requests) add(r.threadId, OF_REQUEST[r.kind]);
  for (const t of threads) {
    if (t.notice?.kind === "provider-signin") add(t.id, "Sign in");
    if (t.state === "failed" || t.notice?.kind === "failed") add(t.id, "Failed");
    if (t.notice?.kind === "interrupted") add(t.id, "Interrupted");
  }
  return threads
    .filter((t) => by.has(t.id) && !snoozeEnd(t, now))
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .map((thread) => ({ thread, reasons: ORDER.filter((r) => by.get(thread.id)!.has(r)) }));
}

/** The reasons as one phrase for a row's second line: "Question and approval", "Failed". */
export function reasonLine(reasons: readonly string[]): string {
  const s = reasons.map((r) => r.toLowerCase()).join(" and ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** A prompt a plugin's durable job is waiting on, listed in the Inbox on its own (it belongs to no thread): a question or an approval, answered where it is. */
export interface JobPrompt {
  request: Request;
  /** The line the row shows: the question's title, or the approval's action. */
  title: string;
  /** Why it is here, in the Inbox's words (`Question`, `Approval`). */
  reason: string;
}

/** The job prompts among the open requests, in the order the server lists them. Cost: one pass. */
export function jobPromptsOf(requests: readonly Request[]): JobPrompt[] {
  return requests.flatMap((r): JobPrompt[] => {
    if (!r.jobId || r.threadId) return [];
    if (r.kind === "question") return [{ request: r, title: r.title, reason: "Question" }];
    if (r.kind === "approval") return [{ request: r, title: r.action || r.title, reason: "Approval" }];
    return [];
  });
}
