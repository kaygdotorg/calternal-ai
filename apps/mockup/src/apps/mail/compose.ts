/** Starting a draft: Reply, Reply all, Forward and Write each make a draft in `DRAFTS` and open it in a window beside the one that is focused (a compose window is a second canvas window, so the message being answered stays readable). */
import { mk } from "../../data/collections";
import { wins } from "../../windows.svelte";
import { DRAFTS, THREADS, type Draft } from "./data";

let n = 0;
export type DraftMode = "new" | "reply" | "all" | "forward";

/** Opens a new draft window. `threadId` is the conversation being answered; `to` addresses a new message (the contact page's Mail button). */
export function startDraft(mode: DraftMode, threadId?: string, to: string[] = []): string {
  const t = threadId ? THREADS[threadId] : undefined, last = t?.msgs.at(-1), id = `draft-${++n}`;
  const them = [...new Set(t?.msgs.filter((m) => !m.mine).map((m) => m.from) ?? [])];
  const d: Draft = { from: t?.account ?? "Personal", to: [], cc: [], subject: "", body: "", atts: [], saved: "", replyTo: undefined };
  if (t && mode !== "new") {
    d.replyTo = threadId; d.from = t.account;
    if (mode === "forward") { d.subject = t.subject.startsWith("Fwd:") ? t.subject : `Fwd: ${t.subject}`; d.atts = last?.atts ?? []; }
    else { d.subject = /^re:/i.test(t.subject) ? t.subject : `Re: ${t.subject}`; d.to = mode === "all" ? them : (last && !last.mine ? [last.from] : them.slice(0, 1)); }
  }
  if (to.length) d.to = to;
  DRAFTS[id] = d;
  mk("mail", id, mode === "new" ? "New message" : d.subject, "", {});
  wins.beside("item", id);
  return id;
}
