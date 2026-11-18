/**
 * Everything this origin may hold from somewhere else. An address can have served another installation, or another app altogether (calternal.cloud served the existing app, which registers a service worker that
 * answers every navigation with its own cached page): none of that may decide what this installation shows. Two rules, both run in `boot.ts` before anything paints:
 *  - while the server says it has no owner yet (`setup: true`), nothing stored here can belong to it: everything is wiped, once, and the page loads again from the network;
 *  - a service worker or a cache on this origin is never ours (this app registers none): they are removed, once, and the page loads again.
 * Each rule has its own one-shot guard in sessionStorage so neither can loop.
 */
const WORKERS = "cal.cleaned.workers", ALL = "cal.cleaned.all";

const guarded = (k: string): boolean => { try { return sessionStorage.getItem(k) === "1"; } catch { return true; } };
const guard = (k: string): void => { try { sessionStorage.setItem(k, "1"); } catch { /* none */ } };
/** Whether this tab has already removed workers and caches. */
export const workersCleaned = (): boolean => guarded(WORKERS);
/** Whether this tab has already wiped the origin. */
export const originWiped = (): boolean => guarded(ALL);

/** Removes every service worker and cache on the origin. True when there was something to remove. */
export async function dropWorkers(): Promise<boolean> {
  let found = false;
  try { for (const r of await navigator.serviceWorker?.getRegistrations?.() ?? []) { found = true; await r.unregister(); } } catch { /* no service workers here */ }
  try { for (const k of await caches?.keys?.() ?? []) { found = true; await caches.delete(k); } } catch { /* no cache storage here */ }
  return found;
}

/** Removes everything stored on the origin: local and session storage, every IndexedDB database, the caches and the service workers. Sets the guard. */
export async function wipeOrigin(): Promise<void> {
  try { localStorage.clear(); sessionStorage.clear(); } catch { /* storage off */ }
  try {
    const dbs = (await indexedDB.databases?.()) ?? [];
    await Promise.all(dbs.map((d) => new Promise<void>((done) => { if (!d.name) return done(); const q = indexedDB.deleteDatabase(d.name); q.onsuccess = q.onerror = q.onblocked = () => done(); })));
  } catch { /* none */ }
  await dropWorkers();
  guard(ALL); guard(WORKERS);
}

/** Marks this tab as having removed workers and caches, so the page can reload once without looping. */
export function markWorkersCleaned(): void { guard(WORKERS); }
