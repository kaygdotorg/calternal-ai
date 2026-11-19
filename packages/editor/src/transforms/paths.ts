/**
 * Links on disk are always standard markdown with angle brackets and a
 * path relative to the linking note, without a leading `./`
 * (ADR 0024): `[Trip ideas](<Trip ideas.md>)`, `[Notes](<../Old/Notes.md>)`.
 */

/** The path of `to` as seen from the folder of `from`. Both are Home paths. */
export function relativePath(from: string, to: string): string {
  const a = from.split('/').slice(0, -1)
  const b = to.split('/')
  let i = 0
  while (i < a.length && i < b.length - 1 && a[i] === b[i]) i++
  const up = a.length - i
  return [...Array<string>(up).fill('..'), ...b.slice(i)].join('/')
}

/** A standard markdown link. Brackets in the title are escaped. */
export function markdownLink(title: string, path: string): string {
  const t = title.replace(/([[\]\\])/g, '\\$1')
  const p = path.replace(/[<>]/g, (c) => encodeURIComponent(c))
  return `[${t}](<${p}>)`
}
