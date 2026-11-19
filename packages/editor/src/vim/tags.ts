/**
 * `it` and `at` in a note (#538). Vim finds a tag pair from the syntax tree, and markdown's tree
 * has inline HTML only as loose tags (`<b>`, `</b>`), never as a pair. So the pair is found in the
 * text: the innermost `<name …>…</name>` around the caret, within a few thousand characters.
 */

import type { CM } from './commands'

const TAG = /<(\/?)([A-Za-z][\w-]*)\b[^<>]*?(\/?)>/g
/** How far either way the pair is looked for. */
const REACH = 4000

type Pos = { line: number; ch: number }

export function tagAround(cm: CM, pos: Pos) {
  const doc = cm.cm6.state.doc
  const line = doc.line(Math.min(pos.line + 1, doc.lines))
  const at = Math.min(line.from + pos.ch, line.to)
  const start = Math.max(0, at - REACH)
  const text = doc.sliceString(start, Math.min(doc.length, at + REACH))
  const open: { name: string; from: number; to: number }[] = []
  let best: { open: [number, number]; close: [number, number] } | null = null
  for (const m of text.matchAll(TAG)) {
    const from = start + m.index!
    const to = from + m[0].length
    if (m[3]) continue // `<br/>` holds nothing
    if (!m[1]) {
      open.push({ name: m[2]!.toLowerCase(), from, to })
      continue
    }
    let i = open.length - 1
    while (i >= 0 && open[i]!.name !== m[2]!.toLowerCase()) i--
    if (i < 0) continue
    const o = open[i]!
    open.length = i
    // Innermost pair holding the caret (on a tag counts, as in Vim).
    if (o.from <= at && at < to && (!best || o.from >= best.open[0])) best = { open: [o.from, o.to], close: [from, to] }
  }
  if (!best) return undefined
  const p = (n: number) => {
    const l = doc.lineAt(n)
    return { line: l.number - 1, ch: n - l.from }
  }
  return { open: { from: p(best.open[0]), to: p(best.open[1]) }, close: { from: p(best.close[0]), to: p(best.close[1]) } }
}
