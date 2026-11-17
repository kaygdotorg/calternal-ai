/**
 * What this device already knows about the signed-in account, so the app can paint at once and let the server confirm: the account's id, whether it holds the server-admin capability, and the list of spaces (the
 * current one is `cal.space`, kept by spaces.svelte.ts). One small record per origin in localStorage; it is only ever a first guess, replaced by the server's answer within one round trip (boot.ts). It is cleared
 * by signing out and when the server says the session ended, so a signed-out person never sees the app from it. Nothing secret: no token, no content.
 */
const KEY = "cal.boot";
/** Older than this, the guess is not trusted and the app waits for the server like a first visit (a long-idle device). */
const MAX_AGE_MS = 24 * 60 * 60 * 1000;

/** The record. */
export interface BootCache { account: string; capabilities: string[]; spaces: { id: string; name: string; role?: string }[]; at: number }

/** The cached record, or null when there is none, it is old, or it is unreadable. */
export function readCache(): BootCache | null {
  try {
    const c = JSON.parse(localStorage.getItem(KEY) ?? "null") as BootCache | null;
    return c && typeof c.account === "string" && Array.isArray(c.spaces) && c.spaces.length && Date.now() - c.at < MAX_AGE_MS ? c : null;
  } catch { return null; }
}
/** Keeps what the server just said. */
export function writeCache(c: Omit<BootCache, "at">): void {
  try { localStorage.setItem(KEY, JSON.stringify({ ...c, at: Date.now() })); } catch { /* private mode: the next visit waits for the server */ }
}
/** Forgets it (signed out, or the session ended). */
export function clearCache(): void { try { localStorage.removeItem(KEY); } catch { /* none */ } }
