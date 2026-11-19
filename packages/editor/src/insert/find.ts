/**
 * Finding block actions by what was typed: the label first, then the other
 * words, then letters in order ("tbl" finds Table). Results come back grouped,
 * the group with the best match first. With nothing typed they come back in
 * the registry's own order, by group.
 */

import { type BlockAction, GROUPS, type Group } from './registry'

const norm = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9 ]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()

/** Letters of `q` in order inside `s`; the score is higher the tighter they sit. */
function loose(s: string, q: string): number {
  let at = -1
  let first = -1
  for (const c of q) {
    at = s.indexOf(c, at + 1)
    if (at < 0) return 0
    if (first < 0) first = at
  }
  // Letters far apart are not a match: at most two stray letters between the first and the last.
  const gap = at - first + 1 - q.length
  return gap > 2 ? 0 : 30 - gap - Math.min(first, 10)
}

/** How well one action answers `q`; 0 means not at all. */
export function score(a: BlockAction, raw: string): number {
  const q = norm(raw)
  if (!q) return 1
  const label = norm(a.label)
  if (label.startsWith(q)) return 100
  if (label.split(' ').some((w) => w.startsWith(q))) return 85
  if (a.keywords.some((k) => norm(k).startsWith(q))) return 75
  if (label.includes(q)) return 60
  if (a.keywords.some((k) => norm(k).includes(q))) return 50
  // Several words: every one of them must hit.
  const parts = q.split(' ')
  if (parts.length > 1) {
    const hay = `${label} ${a.keywords.map(norm).join(' ')}`
    return parts.every((p) => hay.includes(p)) ? 45 : 0
  }
  const l = Math.max(loose(label, q), ...a.keywords.map((k) => loose(norm(k), q)))
  return l > 0 ? Math.min(30, l) : 0
}

export interface Found {
  group: Group
  items: BlockAction[]
}

export function findActions(all: readonly BlockAction[], q: string): Found[] {
  const hit = all.map((a, i) => ({ a, i, s: score(a, q) })).filter((x) => x.s > 0)
  const typed = norm(q) !== ''
  const groups = [...new Set(hit.map((x) => x.a.group))]
  const order = (g: Group) => (GROUPS.indexOf(g) < 0 ? GROUPS.length : GROUPS.indexOf(g))
  const best = (g: Group) => Math.max(...hit.filter((x) => x.a.group === g).map((x) => x.s))
  groups.sort((x, y) => (typed ? best(y) - best(x) || order(x) - order(y) : order(x) - order(y)))
  return groups.map((g) => ({
    group: g,
    items: hit
      .filter((x) => x.a.group === g)
      .sort((x, y) => (typed ? y.s - x.s : 0) || x.i - y.i)
      .map((x) => x.a),
  }))
}
