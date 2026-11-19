/**
 * The saved collections of the current space (`collections.list`), for the Notes sidebar's Collections section. Follows `next_cursor` to the end, even over an empty page (the server may return one), bounded at 20 pages (1,000
 * collections); read again when the space changes and after a collection is made. A failed read leaves the section as it was.
 */
import { listCollections, type CollectionRow } from "./collections";

class CollectionList {
  items = $state.raw<CollectionRow[]>([]);
  private seq = 0;
  /** Reads every page for `spaceId`; a newer call (another space, a refresh) supersedes this one. */
  async load(spaceId: string): Promise<void> {
    const mine = ++this.seq;
    const all: CollectionRow[] = [];
    let cursor: string | undefined;
    try {
      for (let i = 0; i < 20; i++) {
        const page = await listCollections(spaceId, cursor);
        all.push(...page.items);
        cursor = page.next_cursor;
        if (!cursor) break;
      }
    } catch { return; }
    if (mine === this.seq) this.items = all;
  }
}

/** The one list. */
export const collectionList = new CollectionList();
