/**
 * The one definition of a thread's menu: what it offers and what each choice does. The sidebar's right-click, its three-dot button and the swipe's More open it for a row; the thread's own header opens it
 * for the open thread. A person's arrangement (sections) is `layout`; the actions go through the Threads client.
 */
import { toast, undoable, type MenuItem } from "@calternal/ui";
import { href, navigate, segments } from "../../lib/route.svelte";
import { spaces } from "../../shell/spaces.svelte";
import { layout, type Container } from "./layout.svelte";
import type { ThreadSummary } from "./model";
import { client, store } from "./store.svelte";
import { backLabel, snoozeChoices } from "./snooze";
import { snoozeAsk, snoozeAvailable, snoozeThread } from "./snooze.svelte";
import { snoozeEnd } from "./inbox";

/** The ids of the current space's threads that stand on their own in the list (children follow their parent), newest first. */
export function topLevelIds(): string[] {
  const shown = store.threads.filter((t) => t.spaceId === spaces.current.id);
  const known = new Set(shown.map((t) => t.id));
  return shown.filter((t) => !t.parentId || !known.has(t.parentId)).sort((a, b) => b.updatedAt - a.updatedAt).map((t) => t.id);
}
/** The places a thread can be: the unlabelled list (null) and each of the person's sections. */
export const containersOf = (): Container[] => [null, ...layout.sections.map((s) => s.id)];
/** A place's name as the person gave it. */
export const containerName = (c: Container): string => (c === null ? layout.looseName : layout.sections.find((x) => x.id === c)?.name ?? "section");

/** The snooze choices for `t` (none when the server cannot snooze): the presets from now, a date and time, and Unsnooze now when the person snoozed it. */
export function snoozeItems(t: ThreadSummary): MenuItem[] {
  if (!snoozeAvailable()) return [];
  const now = Date.now();
  const s = snoozeEnd(t, now);
  return [
    ...snoozeChoices(now).map((c) => ({ id: `snooze:${c.id}`, label: `Snooze: ${c.label}`, icon: "clock" as const })),
    { id: "snooze:pick", label: "Snooze: pick a date and time…", icon: "clock" as const },
    ...(s && !s.auto ? [{ id: "snooze:off", label: "Unsnooze now" }] : []),
  ];
}

/** The one entry the thread's own menus carry (a long menu turns into a sheet on a phone): it opens the snooze panel with the presets and a date. */
export const snoozeEntry = (t: ThreadSummary): MenuItem[] => (!snoozeAvailable() ? [] : [{ id: "snooze:ask", label: "Snooze…", icon: "clock" as const }, ...(snoozeEnd(t, Date.now()) && !snoozeEnd(t, Date.now())!.auto ? [{ id: "snooze:off", label: "Unsnooze now" }] : [])]);

/** What the menu offers for `t`: rename, move to another section, copy link, stop (while it runs) and archive. */
export function threadItems(t: ThreadSummary): MenuItem[] {
  return [
    { id: "rename", label: "Rename", icon: "edit" as const },
    ...containersOf().filter((c) => c !== layout.containerOf(t.id)).map((c) => ({ id: `move:${c ?? ""}`, label: `Move to ${containerName(c)}`, icon: "folder" as const })),
    ...snoozeEntry(t),
    { id: "copy", label: "Copy link", icon: "link" as const },
    ...(t.state === "running" ? [{ id: "stop", label: "Stop", icon: "stop-round" as const }] : []),
    { id: "archive", label: "Archive", icon: "archive" as const, danger: true },
  ];
}

/** Copies the thread's address to the clipboard and says so. */
export const copyThreadLink = (t: ThreadSummary): void => void navigator.clipboard?.writeText(location.origin + href(`/threads/${t.id}`)).then(() => toast("Link copied"));

/** Archives `t` (leaving it if it is the open one) with an Undo. Rejects when the server refuses. */
export function archiveThread(t: ThreadSummary): Promise<unknown> {
  if (segments()[1] === t.id) navigate("/threads");
  return client.archive(t.id).then(() => undoable("thread.archive", t.id, "Archived", () => client.unarchive(t.id)));
}

/** Does what the person chose from `threadItems`. `rename` starts the rename where the menu was opened (in the row, or in the header). */
export function runThreadItem(id: string, t: ThreadSummary, rename: () => void): void {
  if (id === "rename") rename();
  else if (id.startsWith("move:")) { const to = id.slice(5) || null; const ids = layout.idsOf(to, topLevelIds()); layout.move(t.id, to, ids.length, ids); }
  else if (id.startsWith("snooze:")) {
    const k = id.slice(7);
    if (k === "pick" || k === "ask") snoozeAsk.id = t.id;
    else if (k === "off") snoozeThread(t.id, null);
    else { const c = snoozeChoices(Date.now()).find((x) => x.id === k); if (c) snoozeThread(t.id, c.at, backLabel(c.at, Date.now())); }
  } else if (id === "copy") copyThreadLink(t);
  else if (id === "stop") void client.stop(t.id);
  else if (id === "archive") void archiveThread(t).catch((e: Error) => toast(`Not archived: ${e.message}`));
}
