/**
 * "Shared with me" for notes or collections: what others have shared with the person (`sharing.shared_with_me`), a page at a time, for the Notes sidebar's section and for the header's owner avatar. Opening an item clears its
 * unread dot here at once; the server clears its own mark when the item is read.
 */
import { Paged } from "../paged.svelte";
import { say } from "../../../server/say";
import { sharing, type Incoming } from "./sharing";

class IncomingList {
  constructor(private readonly kind: "notes" | "collections") {}
  /** The pages read so far; `next()` reads the next one when the list is scrolled near its end. */
  readonly pages = new Paged<Incoming>(async (c) => { const r = await sharing.shared_with_me(this.kind, c); return { items: r.items, next: r.next_cursor ?? null }; }, (i) => i.item.id, (e) => say(e, "load what is shared with you"));
  /** What is shared with the person, most recent first. */
  get items(): Incoming[] { return this.pages.items; }
  /** How many have changed since the person last opened them. */
  get unread(): number { return this.items.filter((i) => i.unread).length; }
  /** The share a note arrived by, or undefined when the note is the person's own. */
  of(id: string | undefined): Incoming | undefined { return id ? this.items.find((i) => i.item.id === id) : undefined; }
  /** Marks a note read: the dot goes at once, and the server is told (`sharing.mark_seen`); a failure leaves the server's mark, which the next read of the list brings back. */
  read(id: string): void {
    const i = this.of(id);
    if (!i?.unread) return;
    this.pages.replace(id, { ...i, unread: false });
    void sharing.mark_seen(i.item).catch(() => {});
  }
}

/** The notes shared with the person. */
export const incoming = new IncomingList("notes");
/** The collections shared with the person. */
export const incomingCollections = new IncomingList("collections");
