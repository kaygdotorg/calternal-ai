/**
 * The row images of one open app (revision 13): a bounded cache of object URLs keyed by the host-issued ref, at most `max` images, the least recently used revoked first; reads in flight are shared between rows that want the
 * same ref, capped at `concurrency` at once, and cancelled when the last row that wanted one leaves. Refs that do not have the host's shape are never fetched. Held in memory only and dropped when the lease lapses or the
 * view closes (the catalogue's cache rule).
 */
import { ROW_IMAGE_REF } from "../wire/transport";

/** Reads one image by ref; null when the host has none. */
export type ImageRead = (ref: string, signal?: AbortSignal) => Promise<Blob | null>;

/** What the cache needs to know about key repeats: reads wait while a key repeats and a read in flight is cancelled when one begins (docs/description-contract.md section 8, item 4). */
export interface Hold { readonly held: boolean; subscribe(f: () => void): () => void; subscribeBegin(f: () => void): () => void }

/** The cache. */
export class ImageCache {
  readonly #read: ImageRead; readonly #max: number; readonly #concurrency: number; readonly #hold?: Hold;
  /** Object URLs by ref; `null` remembers a miss (the host had none), so a row that keeps showing its miniature does not ask again on every mount. */
  readonly #urls = new Map<string, string | null>();
  readonly #pending = new Map<string, Entry>();
  #active = 0; readonly #queue: (() => void)[] = []; #wake: (() => void)[] = [];
  readonly #unsub: (() => void)[] = [];
  constructor(read: ImageRead, o: { max?: number; concurrency?: number; hold?: Hold } = {}) {
    this.#read = read; this.#max = o.max ?? 256; this.#concurrency = o.concurrency ?? 4; this.#hold = o.hold;
    if (this.#hold) {
      this.#unsub.push(this.#hold.subscribeBegin(() => { for (const e of this.#pending.values()) e.ctl.abort(); })); // a key began repeating: reads in flight stop
      this.#unsub.push(this.#hold.subscribe(() => { const w = this.#wake; this.#wake = []; for (const f of w) f(); })); // it settled: deferred reads go on
    }
  }

  /** The object URL of the image for `ref`, or null (none, refused, cancelled). `signal` cancels this caller's wait; the read itself stops when no caller waits. */
  get(ref: string, signal?: AbortSignal): Promise<string | null> {
    if (!ROW_IMAGE_REF.test(ref) || signal?.aborted) return Promise.resolve(null);
    if (this.#urls.has(ref)) { const u = this.#urls.get(ref)!; this.#urls.delete(ref); this.#urls.set(ref, u); return Promise.resolve(u); } // most recently used (a remembered miss too)
    let e = this.#pending.get(ref);
    if (!e) { e = { p: null as unknown as Promise<string | null>, waiters: 0, ctl: new AbortController() }; const entry = e; entry.p = this.#run(ref, entry).finally(() => { if (this.#pending.get(ref) === entry) this.#pending.delete(ref); }); this.#pending.set(ref, e); }
    const entry = e; entry.waiters++;
    return new Promise((resolve) => {
      const leave = () => {
        if (--entry.waiters === 0) { entry.ctl.abort(); entry.gone = true; if (this.#pending.get(ref) === entry) this.#pending.delete(ref); } // a row that comes back starts a fresh read, never joins a cancelled one
        resolve(null);
      };
      signal?.addEventListener("abort", leave, { once: true });
      entry.p.then((u) => { signal?.removeEventListener("abort", leave); resolve(u); }, () => resolve(null));
    });
  }

  async #run(ref: string, entry: Entry): Promise<string | null> {
    for (;;) {
      // Not while a key repeats, and not past the concurrency cap.
      while (this.#hold?.held && !entry.gone) await new Promise<void>((r) => this.#wake.push(r));
      if (entry.gone) return null;
      if (this.#active >= this.#concurrency) await new Promise<void>((r) => this.#queue.push(r));
      if (entry.gone) { this.#next(); return null; }
      if (this.#hold?.held) { this.#next(); continue; }
      entry.ctl = new AbortController();
      this.#active++;
      try {
        const blob = await this.#read(ref, entry.ctl.signal);
        if (entry.ctl.signal.aborted) { if (entry.gone) return null; continue; } // cancelled by a key burst while a row still wants it: read again once it settles
        const url = blob ? URL.createObjectURL(blob) : null; // a miss is remembered
        this.#urls.set(ref, url);
        while (this.#urls.size > this.#max) { const [old, u] = this.#urls.entries().next().value as [string, string | null]; this.#urls.delete(old); if (u) URL.revokeObjectURL(u); }
        return url;
      } catch { if (entry.ctl.signal.aborted && !entry.gone) continue; return null; } finally { this.#active--; this.#next(); }
    }
  }
  #next() { this.#queue.shift()?.(); }

  /** Cached entries now (images and remembered misses). */
  get size(): number { return this.#urls.size; }

  /** Drops every image (lease lost, view closed): reads in flight are cancelled, object URLs revoked. */
  clear(): void { for (const e of this.#pending.values()) { e.gone = true; e.ctl.abort(); } this.#pending.clear(); for (const u of this.#urls.values()) if (u) URL.revokeObjectURL(u); this.#urls.clear(); }

  /** Stops listening to key repeats (the view closed). */
  destroy(): void { this.clear(); for (const f of this.#unsub) f(); this.#unsub.length = 0; }
}

interface Entry { p: Promise<string | null>; waiters: number; ctl: AbortController; gone?: boolean }
