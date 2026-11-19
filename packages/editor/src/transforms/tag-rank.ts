/**
 * Which tags the tag pickers offer for what was typed (#449). The `#` list in
 * the editor and the "+" picker under a note's title share this one matcher.
 *
 * Tags nest with `/`. A tag matches what was typed when
 *  - its whole path starts with it (`area/w` finds `area/work`), or
 *  - any one part of the path starts with it (`fit` finds `area/fitness`), or
 *  - what was typed holds a `/` and the path holds it starting at a part
 *    (`a/fit` finds `goals/a/fitness`).
 *
 * Order: the exact tag, then a path that starts with it, then a tag whose
 * last part is exactly it (`area/fitness` for `fitness`), then a match on
 * another part. Inside each: more notes first, then A to Z.
 *
 * It stays instant for ten thousand tags: each tag is split and lowercased
 * once, when the list changes (see {@link tagIndex}), and a keystroke only
 * walks that list, keeping the few best rows as it goes.
 */

/** One row of the picker. */
export interface TagChoice {
  /** The whole tag, like `area/work`. */
  tag: string
  /** Notes that use the tag or a tag inside it. */
  count: number
  /** Whether tags sit inside it. */
  kids: boolean
  /** The letters of `tag` that matched, as [from, to), to draw them stronger. */
  match: [number, number]
}

/** A tag, prepared once. */
interface Entry {
  tag: string
  low: string
  /** Where each part of the path starts, in `low`. */
  starts: number[]
  count: number
  kids: boolean
}

export type TagIndex = readonly Entry[]

const made = new WeakMap<object, TagIndex>()

/** Prepares tags for matching. `counts` is every tag with its number of notes. */
export function tagIndex(counts: Iterable<[string, number]>): TagIndex {
  const parents = new Set<string>()
  const out: Entry[] = []
  for (const [tag, count] of counts) {
    const low = tag.toLowerCase()
    const starts = [0]
    for (let i = low.indexOf('/'); i >= 0; i = low.indexOf('/', i + 1)) {
      starts.push(i + 1)
      parents.add(tag.slice(0, i))
    }
    out.push({ tag, low, starts, count, kids: false })
  }
  for (const e of out) e.kids = parents.has(e.tag)
  return out
}

/** {@link tagIndex} again only when the map itself is a new one (the app makes a new one when tags change). */
export function tagIndexOf(counts: ReadonlyMap<string, number>): TagIndex {
  let ix = made.get(counts)
  if (!ix) made.set(counts, (ix = tagIndex(counts)))
  return ix
}

const AT = 1 << 20

/** How well `e` fits `q`, as `rank * AT + at` (rank 1 is best), or 0 when it does not; `at` is where the match sits. */
function fit(e: Entry, q: string): number {
  const low = e.low
  if (low.startsWith(q)) return (low.length === q.length ? 1 : 2) * AT
  const starts = e.starts
  const last = starts.length - 1
  let other = -1
  for (let i = 1; i <= last; i++) {
    const s = starts[i]!
    if (!low.startsWith(q, s)) continue
    // The last part, exactly.
    if (i === last && low.length - s === q.length) return 3 * AT + s
    if (other < 0) other = s
  }
  return other >= 0 ? 4 * AT + other : 0
}

/** Whether (rank, e) beats (rank2, e2): better rank, more notes, then A to Z. */
function better(rank: number, e: Entry, rank2: number, e2: Entry): boolean {
  if (rank !== rank2) return rank < rank2
  if (e.count !== e2.count) return e.count > e2.count
  return e.low !== e2.low ? e.low < e2.low : e.tag < e2.tag
}

/** The best `limit` rows for `typed`, never a tag `skip` holds. */
export function matchTags(index: TagIndex, typed: string, limit = 7, skip?: ReadonlySet<string>): TagChoice[] {
  const q = typed.toLowerCase()
  const ranks: number[] = []
  const rows: Entry[] = []
  const ats: number[] = []
  for (let n = 0; n < index.length; n++) {
    const row = index[n]!
    const f = fit(row, q)
    if (!f) continue
    const rank = Math.floor(f / AT)
    const full = rows.length === limit
    if (full && !better(rank, row, ranks[limit - 1]!, rows[limit - 1]!)) continue
    if (skip?.has(row.tag)) continue
    let i = rows.length
    while (i > 0 && better(rank, row, ranks[i - 1]!, rows[i - 1]!)) i--
    ranks.splice(i, 0, rank)
    rows.splice(i, 0, row)
    ats.splice(i, 0, f % AT)
    if (full) {
      ranks.pop()
      rows.pop()
      ats.pop()
    }
  }
  return rows.map((row, i) => ({ tag: row.tag, count: row.count, kids: row.kids, match: [ats[i]!, ats[i]! + q.length] }))
}

/**
 * The rows for `typed` (what follows `#`). `counts` holds every tag, and
 * every tag above a nested one (`area` for `area/work`), with its number
 * of notes.
 */
export function rankTags(counts: ReadonlyMap<string, number>, typed: string, limit = 7): TagChoice[] {
  return matchTags(tagIndexOf(counts), typed, limit)
}
