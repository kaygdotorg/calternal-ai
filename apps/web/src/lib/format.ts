/**
 * Small formatting and clipboard helpers shared by the screens that show server data (Settings, Notes), so a date, a role name or a copied link looks and behaves the same everywhere. Pure apart from the clipboard;
 * dates use the person's locale and time zone, never a fixed one.
 */

/** A day as "Nov 17, 2026" in the person's locale, from Unix milliseconds or an ISO string; `fallback` (default "never") for null or undefined. */
export function shortDate(when: number | string | null | undefined, fallback = "never"): string {
  if (when === null || when === undefined || when === "") return fallback;
  return new Date(when).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

/** A day and time as "Nov 17, 2:05 PM", for things that happened recently (a deletion, a code that expires). */
export function shortDateTime(when: number | string): string {
  return new Date(when).toLocaleString(undefined, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" });
}

/** The time of day as "2:05:09 PM" (a code's expiry). */
export function clockTime(ms: number): string { return new Date(ms).toLocaleTimeString(); }

/** A word with its first letter upper-cased ("editor" becomes "Editor"): roles and states shown as labels. */
export function capitalise(word: string): string { return word ? word[0]!.toUpperCase() + word.slice(1) : word; }

/**
 * Puts text on the clipboard. Resolves true when it was copied and false when the browser refused (no permission, an insecure page); the caller then says so in words, so a failed copy is never silent.
 */
export async function copyText(text: string): Promise<boolean> {
  try { await navigator.clipboard.writeText(text); return true; } catch { return false; }
}
