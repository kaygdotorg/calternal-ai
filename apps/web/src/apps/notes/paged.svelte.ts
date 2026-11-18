/**
 * A list read from the server one page at a time and kept current: the first page is read again on every change, the pages already scrolled past are kept, and the next page is read when the person gets near the end
 * (`next`, called by the `reach` action). Rows are matched by `key`, so a row that moved to the top is not listed twice. A row removed or changed on another device and already scrolled past stays shown until the
 * list is read from the start again (a space switch, a filter, Try again); `drop` removes a row this client removed itself.
 */
import type { Page } from "./client";

/** A page reader: `cursor` is the previous page's `next` (none for the first page). */
export type ReadPage<T> = (cursor?: string) => Promise<Page<T>>;

/** A paged list of rows of type `T`; reactive (`items`, `more`, `error`). One read per call; see the file header for what is kept across reads. */
export class Paged<T> {
  /** The rows read so far, in the server's order. */
  items = $state<T[]>([]);
  /** There is another page to read. */
  more = $state(false);
  /** The words for a failed read (empty when none); `next` does nothing until `retry`. */
  error = $state("");
  private cursor: string | null = null;
  private tail = false;
  private seq = 0;
  private busy = false;
  private recovered = false;

  /** `read` fetches a page; `key` identifies a row; `fail` words an error. */
  constructor(private read: ReadPage<T>, private key: (row: T) => string, private fail: (e: unknown) => string = (e) => String(e)) {}

  /** Reads the first page again (after a change) and keeps the pages scrolled past. Throws when the first page cannot be read. Cost: one request. */
  async refresh(): Promise<void> {
    const mine = ++this.seq;
    const head = await this.read();
    if (mine !== this.seq) return; // a newer read (or `reset`) owns the list
    if (this.tail) {
      const seen = new Set(head.items.map(this.key));
      this.items = [...head.items, ...this.items.filter((r) => !seen.has(this.key(r)))];
    } else {
      this.items = head.items; this.cursor = head.next;
    }
    this.more = this.cursor !== null; this.error = "";
  }

  /** Starts over from the first page (another space or filter): the rows shown go away at once. */
  async reset(): Promise<void> { this.clear(); await this.refresh(); }

  /** Forgets every row and cursor without reading (the list is not shown now; the next `refresh` reads it). */
  clear(): void { this.seq++; this.items = []; this.cursor = null; this.tail = false; this.more = false; this.error = ""; }

  /** Reads the next page when there is one. A cursor the server no longer honours (it restarted) is answered by starting over once; a second failure stops and says why. */
  async next(): Promise<void> {
    if (this.busy || this.cursor === null || this.error) return;
    this.busy = true;
    const mine = this.seq, cursor = this.cursor;
    try {
      const page = await this.read(cursor);
      if (mine !== this.seq) return;
      const seen = new Set(this.items.map(this.key));
      this.items = [...this.items, ...page.items.filter((r) => !seen.has(this.key(r)))];
      this.cursor = page.next; this.tail = true; this.more = page.next !== null; this.recovered = false;
    } catch (e) {
      if (mine !== this.seq) return;
      if (!this.recovered) { this.recovered = true; this.busy = false; await this.reset().catch((x) => { this.error = this.fail(x); }); return; }
      this.error = this.fail(e);
    } finally { this.busy = false; }
  }

  /** Clears a failure and reads the next page again. */
  async retry(): Promise<void> { this.error = ""; await this.next(); }

  /** Whether a row with this id is loaded. */
  has(id: string): boolean { return this.items.some((r) => this.key(r) === id); }

  /** Replaces a loaded row with a newer copy of it (a note renamed on another device), keeping its place. */
  replace(id: string, row: T): void { this.items = this.items.map((r) => (this.key(r) === id ? row : r)); }

  /** Removes a row this client removed itself, so it does not wait for the next full read. */
  drop(id: string): void { this.items = this.items.filter((r) => this.key(r) !== id); }
}
