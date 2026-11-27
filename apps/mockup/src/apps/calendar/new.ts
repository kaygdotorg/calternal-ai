/** The sidebar's "New event": makes an event item that the form (not the event page) draws, and opens it in a window beside the focused one. */
import { mk } from "../../data/collections";
import { wins } from "../../windows.svelte";
import { TODAY } from "./dates";

const fresh = new Set<string>(); let n = 0;
/** Whether an item is an event being written. */
export const isNewEvent = (id: string): boolean => fresh.has(id);
/** Opens a new-event window for `day` (today if none). */
export function startEvent(day: string = TODAY): string {
  const id = `new-event-${++n}`; fresh.add(id); mk("event", id, "New event", "", { date: day }); wins.beside("item", id); return id;
}
