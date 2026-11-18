import { toast, undoable } from "@calternal/ui";
import { client, store } from "./store.svelte";

/** The thread whose "Pick a date and time" panel is open (SnoozePicker.svelte), or null. One flag, set from the Inbox row's menu and the thread's menu. */
export const snoozeAsk = $state<{ id: string | null }>({ id: null });

/** Whether the server can snooze at all (the client has `snooze`): every snooze control, menu entry and swipe action asks this, so none shows where it could not work. */
export const snoozeAvailable = (): boolean => typeof client.snooze === "function";

/** Snoozes a thread until `until`, with an Undo; null brings it back now. Says so when the server refuses. */
export function snoozeThread(id: string, until: number | null, label = ""): void {
  store.now = Date.now();
  const before = store.get(id)?.snoozedUntil ?? null;
  if (!snoozeAvailable()) return;
  void client.snooze!(id, until)
    .then(() => { if (until !== null) undoable("thread.snooze", id, label ? `Snoozed: ${label.replace(/^Back /, "back ")}` : "Snoozed", () => client.snooze!(id, before).then(() => true)); })
    .catch((e: Error) => toast(`Not snoozed: ${e.message}`));
}
