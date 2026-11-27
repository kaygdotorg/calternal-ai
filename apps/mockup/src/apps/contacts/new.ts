/** The sidebar's "New contact": makes a contact item that the form draws, opened in a window beside the focused one. */
import { mk } from "../../data/collections";
import { wins } from "../../windows.svelte";

const fresh = new Set<string>(); let n = 0;
/** Whether an item is a contact being written. */
export const isNewContact = (id: string): boolean => fresh.has(id);
/** Opens a new-contact window. */
export function startContact(): string { const id = `new-contact-${++n}`; fresh.add(id); mk("contact", id, "New contact", ""); wins.beside("item", id); return id; }
