/**
 * Which rows the `[[` and `@` picker offers for what was typed (#448).
 *
 * In this order, and nothing else above them:
 *
 * 1. **Notes whose title matches**: the exact title, then titles that start
 *    with it, then titles with a word that starts with it, then titles that
 *    hold it, then (three letters or more) titles holding its letters in
 *    order, close together, then titles the search matched forgiving typos.
 *    Inside a tier, what the person links and opens
 *    often comes first (`lead`), then what changed last. A day note matches
 *    by its title and by its ISO date.
 * 2. **Link targets nobody has made yet** whose name matches: other notes
 *    link to them, so they will be notes one day. Choosing one writes the
 *    same link, so it stays the same future note.
 * 3. **"Create"** for exactly what was typed, unless a note or a target
 *    already has that name.
 * 4. **Mentioned in**: notes that hold the words in their text but not in
 *    their title. Only while there are fewer than {@link FEW_TITLES} title
 *    matches, never above them, and few.
 *
 * It runs on every keystroke over every note, so it keeps to one pass over
 * ready-made folded titles and a short list of the best so far.
 */

import type { LinkTarget } from './links'

/** One note, ready to be matched: its title folded once, when the list changed, not per keystroke. */
export interface TitleEntry {
  id: string
  title: string
  /** The title, folded ({@link fold}). */
  key: string
  /** A day note's ISO day (`2026-05-01`), which matches too. */
  day?: string
  /** Home path, like `Notes/Trip.md`. */
  path: string
  /** When it last changed (ISO), for ties. */
  edited: string
}

/** A link target no note holds yet, as the server lists it. */
export interface MissingTarget {
  title: string
  /** Home path it would live at. */
  path: string
  /** Notes linking to it. */
  count: number
}

export interface LinkQuery {
  /** What was typed after `[[` or `@`. */
  query: string
  notes: readonly TitleEntry[]
  /** The note being edited: never offered to itself. */
  skip?: string
  missing?: readonly MissingTarget[]
  /** How the person's own use ranks notes (0 = best), by id: breaks ties inside a tier. */
  lead?: ReadonlyMap<string, number>
  /** Notes the search found by their title with typos forgiven (`Keshve`): after every other title match. */
  forgiven?: readonly TitleEntry[]
  /** Notes that hold the words in their text, best first. */
  mentions?: readonly LinkTarget[]
  /** Where a note made for `title` would live, or null if a note already is there. */
  createAt?: (title: string) => string | null
}

/** Fewer title matches than this, and notes that only mention the words are offered too. */
export const FEW_TITLES = 5
const MAX_TITLES = 6
const MAX_MISSING = 4
const MAX_MENTIONS = 4

/** The comparison form: Unicode NFC, lowercase. */
export const fold = (s: string): string => s.normalize('NFC').toLowerCase()

/** Whether the character at `i` of `s` is part of a word (any letter or digit; other scripts count). */
function wordChar(s: string, i: number): boolean {
  const c = s.charCodeAt(i)
  if (c < 128) return (c >= 48 && c <= 57) || (c >= 97 && c <= 122) || (c >= 65 && c <= 90)
  return /[\p{L}\p{N}]/u.test(s[i]!)
}

/** Whether `word` starts a word of `key`, looking from `from` on. */
function startsAWord(key: string, word: string, from = 0): boolean {
  for (let at = key.indexOf(word, from); at >= 0; at = key.indexOf(word, at + 1)) if (at === 0 || !wordChar(key, at - 1)) return true
  return false
}

/** Whether the letters of `q` are in `key` in order, within a stretch not much longer than `q`. */
function nearlyHolds(key: string, q: string): boolean {
  const room = q.length * 2 + 2
  for (let from = key.indexOf(q[0]!); from >= 0 && from + q.length <= key.length; from = key.indexOf(q[0]!, from + 1)) {
    let at = from
    let i = 1
    while (i < q.length) {
      at = key.indexOf(q[i]!, at + 1)
      if (at < 0 || at - from >= room) break
      i++
    }
    if (i === q.length) return true
  }
  return false
}

/** 0 the exact name, 1 starts with it, 2 words start with its words, 3 holds it; null no match (see {@link nearlyHolds} for the rest). */
export function tierOf(key: string, q: string, words: readonly string[]): number | null {
  if (key === q) return 0
  const at = key.indexOf(q)
  if (at === 0) return 1
  if (words.length === 1) {
    if (at < 0) return null
    return startsAWord(key, q, at) ? 2 : 3
  }
  if (words.every((w) => startsAWord(key, w))) return 2
  return at > 0 ? 3 : null
}

interface Found {
  e: TitleEntry
  tier: number
  lead: number
}

/** Better first: tier, then the person's own use, then the newest change, then A to Z. */
function better(a: Found, b: Found): boolean {
  if (a.tier !== b.tier) return a.tier < b.tier
  if (a.lead !== b.lead) return a.lead < b.lead
  if (a.e.edited !== b.e.edited) return a.e.edited > b.e.edited
  return a.e.key < b.e.key
}

/**
 * The best `limit` notes whose title (or day) matches `typed`, best first, and whether any note
 * (the skipped one too) is called exactly that.
 */
function scan(
  notes: readonly TitleEntry[],
  q: string,
  opts: { skip?: string; lead?: ReadonlyMap<string, number>; limit: number; forgiven?: readonly TitleEntry[] },
): { top: TitleEntry[]; exact: boolean } {
  const words = q.split(/\s+/)
  const best: Found[] = []
  let exact = false
  const offer = (e: TitleEntry, tier: number) => {
    if (tier === 0) exact = true
    if (e.id === opts.skip) return
    // Worse than the worst of a full list: no need to look closer.
    if (best.length === opts.limit && tier > best[opts.limit - 1]!.tier) return
    const f: Found = { e, tier, lead: opts.lead?.get(e.id) ?? Infinity }
    if (best.length === opts.limit && !better(f, best[opts.limit - 1]!)) return
    let i = best.length
    while (i > 0 && better(f, best[i - 1]!)) i--
    best.splice(i, 0, f)
    if (best.length > opts.limit) best.pop()
  }
  // A day's ISO date can only be what was typed if digits were.
  const digits = /\d/.test(q)
  const plain = (e: TitleEntry): number | null => {
    const t = tierOf(e.key, q, words)
    if (!e.day || !digits) return t
    const d = tierOf(e.day, q, words)
    return d !== null && (t === null || d < t) ? d : t
  }
  for (const e of notes) {
    const t = plain(e)
    if (t !== null) offer(e, t)
  }
  // Only when the exact kinds of match fill fewer rows than are shown: the letters in order, then the search's own.
  if (best.length < opts.limit && q.length >= 3) {
    for (const e of notes) if (plain(e) === null && nearlyHolds(e.key, q)) offer(e, 4)
  }
  if (best.length < opts.limit) {
    for (const e of opts.forgiven ?? []) if (plain(e) === null && !(q.length >= 3 && nearlyHolds(e.key, q))) offer(e, 5)
  }
  return { top: best.map((f) => f.e), exact }
}

/** The best `limit` notes whose title (or day) matches `typed`, best first. */
export function rankTitles(
  notes: readonly TitleEntry[],
  typed: string,
  opts: { skip?: string; lead?: ReadonlyMap<string, number>; limit?: number } = {},
): TitleEntry[] {
  const q = fold(typed.trim())
  return q ? scan(notes, q, { ...opts, limit: opts.limit ?? MAX_TITLES }).top : []
}

/** The rows for what was typed, in the order the picker shows them. */
export function rankLinks(input: LinkQuery): LinkTarget[] {
  const typed = input.query.trim()
  const q = fold(typed)
  if (!q) return []
  const { top: found, exact } = scan(input.notes, q, { skip: input.skip, lead: input.lead, limit: MAX_TITLES, forgiven: input.forgiven })
  const rows: LinkTarget[] = found.map((e) => ({ kind: 'note', id: e.id, title: e.title, path: e.path, daily: !!e.day }))

  const missing = (input.missing ?? []).slice(0, MAX_MISSING)
  for (const m of missing) rows.push({ kind: 'missing', title: m.title, path: m.path, count: m.count })

  // "Create" for exactly what was typed, unless that name is taken: by a note (even this one), or a target.
  const taken = exact || missing.some((m) => fold(m.title) === q)
  const path = taken ? null : (input.createAt?.(typed) ?? null)
  if (path) rows.push({ kind: 'create', title: typed, path })

  if (found.length + missing.length < FEW_TITLES) {
    const have = new Set(found.map((e) => e.id))
    let n = 0
    for (const m of input.mentions ?? []) {
      if ((m.id && have.has(m.id)) || m.id === input.skip) continue
      rows.push({ ...m, kind: 'mention' })
      if (++n === MAX_MENTIONS) break
    }
  }
  return rows
}
