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
const OF_REQUEST: Record<Request["kind"], string> = { question: "Question", approval: "Approval", secret: "Secret" };

/** The Inbox for these threads and their open requests. Cost: one pass over each. */
export function inboxOf(threads: readonly ThreadSummary[], requests: readonly Request[]): InboxItem[] {
  const by = new Map<string, Set<string>>();
  const add = (id: string, reason: string) => (by.get(id) ?? by.set(id, new Set()).get(id)!).add(reason);
  for (const r of requests) add(r.threadId, OF_REQUEST[r.kind]);
  for (const t of threads) {
    if (t.notice?.kind === "provider-signin") add(t.id, "Sign in");
    if (t.state === "failed" || t.notice?.kind === "failed") add(t.id, "Failed");
    if (t.notice?.kind === "interrupted") add(t.id, "Interrupted");
  }
  return threads
    .filter((t) => by.has(t.id))
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .map((thread) => ({ thread, reasons: ORDER.filter((r) => by.get(thread.id)!.has(r)) }));
}

/** The reasons as one phrase for a row's second line: "Question and approval", "Failed". */
export function reasonLine(reasons: readonly string[]): string {
  const s = reasons.map((r) => r.toLowerCase()).join(" and ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}
