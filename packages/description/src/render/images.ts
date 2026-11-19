/**
 * The row images of one open app (revision 13): a bounded cache of object URLs keyed by the host-issued ref, at most `max` images, the least recently used revoked first; reads in flight are shared between rows that want the
 * same ref, capped at `concurrency` at once, and cancelled when the last row that wanted one leaves. Refs that do not have the host's shape are never fetched. Held in memory only and dropped when the lease lapses or the
 * view closes (the catalogue's cache rule).
 */
import { ROW_IMAGE_REF } from "../wire/transport";

/** Reads one image by ref; null when the host has none. */
export type ImageRead = (ref: string, signal?: AbortSignal) => Promise<Blob | null>;

/** The cache. */
export class ImageCache {
  readonly #read: ImageRead; readonly #max: number; readonly #concurrency: number;
  readonly #urls = new Map<string, string | null>();
  readonly #pending = new Map<string, { p: Promise<string | null>; waiters: number; ctl: AbortController }>();
  #active = 0; readonly #queue: (() => void)[] = [];
  constructor(read: ImageRead, o: { max?: number; concurrency?: number } = {}) { this.#read = read; this.#max = o.max ?? 256; this.#concurrency = o.concurrency ?? 4; }

  /** The object URL of the image for `ref`, or null (none, refused, cancelled). `signal` cancels this caller's wait; the read itself stops when no caller waits. */
  get(ref: string, signal?: AbortSignal): Promise<string | null> {
    if (!ROW_IMAGE_REF.test(ref) || signal?.aborted) return Promise.resolve(null);
    if (this.#urls.has(ref)) { const u = this.#urls.get(ref)!; this.#urls.delete(ref); this.#urls.set(ref, u); return Promise.resolve(u); } // most recently used
    let e = this.#pending.get(ref);
    if (!e) {
      const ctl = new AbortController();
      const p = this.#run(ref, ctl.signal).finally(() => this.#pending.delete(ref));
      e = { p, waiters: 0, ctl }; this.#pending.set(ref, e);
    }
    const entry = e; entry.waiters++;
    return new Promise((resolve) => {
      const leave = () => { if (--entry.waiters === 0) entry.ctl.abort(); resolve(null); };
      signal?.addEventListener("abort", leave, { once: true });
      entry.p.then((u) => { signal?.removeEventListener("abort", leave); entry.waiters = Math.max(0, entry.waiters); resolve(u); }, () => resolve(null));
    });
  }

  async #run(ref: string, signal: AbortSignal): Promise<string | null> {
    if (this.#active >= this.#concurrency) await new Promise<void>((r) => this.#queue.push(r));
    if (signal.aborted) { this.#next(); return null; }
    this.#active++;
    try {
      const blob = await this.#read(ref, signal);
      if (!blob || signal.aborted) return null;
      const url = URL.createObjectURL(blob);
      this.#urls.set(ref, url);
      while (this.#urls.size > this.#max) { const [old, u] = this.#urls.entries().next().value as [string, string | null]; this.#urls.delete(old); if (u) URL.revokeObjectURL(u); }
      return url;
    } catch { return null; } finally { this.#active--; this.#next(); }
  }
  #next() { this.#queue.shift()?.(); }

  /** Cached images now. */
  get size(): number { return this.#urls.size; }

  /** Drops every image (lease lost, view closed): reads in flight are cancelled, object URLs revoked. */
  clear(): void { for (const e of this.#pending.values()) e.ctl.abort(); this.#pending.clear(); for (const u of this.#urls.values()) if (u) URL.revokeObjectURL(u); this.#urls.clear(); }
}
