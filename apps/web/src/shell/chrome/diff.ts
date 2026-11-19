/**
 * What a new description changes compared with the last one sent, for the publisher to decide whether the shell needs it NOW (native-chrome.svelte.ts). Both work on the wire form (`toSchema` output, plain JSON). Cost: one
 * structured copy and one `JSON.stringify` per call, on a description of at most a few hundred rows (the windows cap long sections), once per frame at most.
 */
import type { ChromeBadge, ChromeDescription } from "./types";

/** A copy with `fix` applied to every part of the description it names. */
function normalised(d: ChromeDescription, fix: (c: ChromeDescription) => void): string {
  const c = structuredClone(d); c.revision = 0; fix(c); return JSON.stringify(c);
}

/**
 * Whether `next` differs from `prev` ONLY in which row is selected (the `selected` flags), and that row is `rowId`, the one the shell itself just chose with a `row` event: the shell already shows its own selection, so
 * the echo is not worth a full apply. The header title is NOT part of this: it changes when the selected thing opens and the shell may draw it from the description, so a changed title is still sent (the conservative choice).
 */
export function selectionEchoOnly(prev: ChromeDescription, next: ChromeDescription, rowId: string): boolean {
  const flags = new Set<string>();
  for (const s of next.sidebar.sections) for (const r of s.rows) if (r.selected) flags.add(r.id);
  if (flags.size > 1 || (flags.size === 1 && !flags.has(rowId))) return false;
  const strip = (c: ChromeDescription) => { for (const s of c.sidebar.sections) for (const r of s.rows) delete r.selected; };
  return normalised(prev, strip) === normalised(next, strip);
}

/**
 * Whether `next` differs from `prev` ONLY in counts: a row's or selector option's count badge value and a section's `count` (the length of its list). Under churn these move several times a second while a key repeats; the
 * publisher holds such a publication back (at most one a second) instead of applying a whole description for each.
 */
export function countsOnly(prev: ChromeDescription, next: ChromeDescription): boolean {
  const zero = (b: ChromeBadge) => { if (b.kind === "count") b.value = 0; };
  const strip = (c: ChromeDescription) => {
    for (const s of c.sidebar.sections) { delete s.count; for (const r of s.rows) r.trailing?.forEach(zero); }
    for (const sel of c.sidebar.selectors) for (const o of sel.options) if (o.badge) zero(o.badge);
  };
  return normalised(prev, strip) === normalised(next, strip);
}
