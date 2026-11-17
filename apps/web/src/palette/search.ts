/**
 * The palette's search: typo tolerant, over lower-cased titles built once when the data changes, never on a keystroke. A query is words; every word has to
 * find a word of the title: exactly, as a prefix, inside it, or within one typo (an inserted, dropped, changed or swapped letter, for words of four letters or
 * more). Scores add up; the best come first, ties go to the shorter title. Cost per query: one pass over the items, no allocation per item beyond the result list.
 */

/** Something the palette can find. */
export interface Item {
  id: string;
  kind: "thread" | "note" | "command";
  title: string;
  /** The space it is in (shown when it is not the current one). */
  space: string;
  /** Where choosing it goes. */
  go?: string;
  op?: string;
  /** A small label: the app or section. */
  group?: string;
  /** Extra words that also find it. */
  words?: string[];
}

interface Entry { item: Item; words: string[]; text: string }
/** The index: built from the items once. */
export interface Index { entries: Entry[] }

const split = (s: string) => s.toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean);

/** Builds the index. Call when the data changes. */
export function buildIndex(items: Item[]): Index {
  return { entries: items.map((item) => ({ item, words: split(item.title + " " + (item.words ?? []).join(" ")), text: item.title.toLowerCase() })) };
}

/** True when `a` and `b` differ by one insertion, deletion, substitution or adjacent swap. */
function oneEdit(a: string, b: string): boolean {
  if (a === b) return true;
  const d = a.length - b.length;
  if (d > 1 || d < -1) return false;
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  if (d === 0) return a.slice(i + 1) === b.slice(i + 1) || (a[i] === b[i + 1] && a[i + 1] === b[i] && a.slice(i + 2) === b.slice(i + 2));
  return d === 1 ? a.slice(i + 1) === b.slice(i) : b.slice(i + 1) === a.slice(i);
}

function wordScore(q: string, words: string[]): number {
  let best = 0;
  for (const w of words) {
    if (w === q) return 4;
    if (w.startsWith(q)) best = Math.max(best, 3);
    else if (q.length >= 3 && w.includes(q)) best = Math.max(best, 1.5);
    else if (q.length >= 4 && (oneEdit(q, w) || (w.length > q.length && oneEdit(q, w.slice(0, q.length))))) best = Math.max(best, 1);
  }
  return best;
}

/** The items matching `query`, best first (at most `limit`). An empty query matches nothing. */
export function search(index: Index, query: string, limit = 12): Item[] {
  const qs = split(query);
  if (!qs.length) return [];
  const hits: { item: Item; score: number }[] = [];
  for (const e of index.entries) {
    let score = 0;
    for (const q of qs) {
      const s = wordScore(q, e.words);
      if (!s) { score = 0; break; }
      score += s;
    }
    if (score) hits.push({ item: e.item, score: score - e.text.length / 400 });
  }
  hits.sort((a, b) => b.score - a.score);
  return hits.slice(0, limit).map((h) => h.item);
}
