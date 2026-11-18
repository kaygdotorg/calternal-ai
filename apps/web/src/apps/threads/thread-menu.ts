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

/** The ids of the current space's threads that stand on their own in the list (children follow their parent; the attention card's first three are listed there instead), newest first. */
export function topLevelIds(): string[] {
  const hidden = new Set(store.attention.slice(0, 3).map((a) => a.thread.id));
  const shown = store.threads.filter((t) => !hidden.has(t.id) && t.spaceId === spaces.current.id);
  const known = new Set(shown.map((t) => t.id));
  return shown.filter((t) => !t.parentId || !known.has(t.parentId)).sort((a, b) => b.updatedAt - a.updatedAt).map((t) => t.id);
}
/** The places a thread can be: the unlabelled list (null) and each of the person's sections. */
export const containersOf = (): Container[] => [null, ...layout.sections.map((s) => s.id)];
/** A place's name as the person gave it. */
export const containerName = (c: Container): string => (c === null ? layout.looseName : layout.sections.find((x) => x.id === c)?.name ?? "section");

/** What the menu offers for `t`: rename, move to another section, copy link, stop (while it runs) and archive. */
export function threadItems(t: ThreadSummary): MenuItem[] {
  return [
    { id: "rename", label: "Rename" },
    ...containersOf().filter((c) => c !== layout.containerOf(t.id)).map((c) => ({ id: `move:${c ?? ""}`, label: `Move to ${containerName(c)}` })),
    { id: "copy", label: "Copy link" },
    ...(t.state === "running" ? [{ id: "stop", label: "Stop" }] : []),
    { id: "archive", label: "Archive", danger: true },
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
  else if (id === "copy") copyThreadLink(t);
  else if (id === "stop") void client.stop(t.id);
  else if (id === "archive") void archiveThread(t).catch((e: Error) => toast(`Not archived: ${e.message}`));
}
