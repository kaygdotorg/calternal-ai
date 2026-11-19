/**
 * The list of hidden blocks in a note's frontmatter (ADR 0056, #611): `hidden blocks: ["^id", …]`.
 *
 * Pure text functions. Reading takes the union of `hidden blocks:` and an older `hidden:` list
 * (a plain `hidden: true` is the note's own flag and is never a list). Writing only touches
 * `hidden blocks:`, and moves an old `hidden:` list into it in the same edit. No other line moves.
 */

export const KEY = 'hidden blocks'
const OLD = 'hidden'

export interface Edit {
  from: number
  to: number
  insert: string
}

interface Field {
  key: string
  /** From the start of the key line to the end of the field's last line (with its line break). */
  from: number
  to: number
  ids: string[]
}

const FRONT = /^---\r?\n([\s\S]*?\r?\n)?---[ \t]*\r?\n/
const ID = /\^([A-Za-z0-9-]{2,})/g

function fields(text: string): { fields: Field[]; close: number } | null {
  const fm = FRONT.exec(text)
  if (!fm) return null
  const body = fm[1] ?? ''
  const close = 4 + body.length
  const lines = body.split('\n')
  const out: Field[] = []
  let at = 4
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!
    const m = /^(hidden blocks|hidden):[ \t]*(.*?)\r?$/.exec(line)
    if (!m) {
      at += line.length + 1
      continue
    }
    let j = i + 1
    let value = m[2]!
    if (value === '') {
      while (j < lines.length && /^[ \t]*-/.test(lines[j]!)) value += ' ' + lines[j++]
    } else if (value.startsWith('[') && !value.includes(']')) {
      while (j < lines.length) {
        value += ' ' + lines[j]
        if (lines[j++]!.includes(']')) break
      }
    }
    const end = lines.slice(0, j).reduce((n, l) => n + l.length + 1, 4)
    if (!/^(true|false)$/i.test(value.trim())) {
      out.push({ key: m[1]!, from: at, to: Math.min(end, close), ids: [...value.matchAll(ID)].map((x) => x[1]!) })
    }
    at = end
    i = j - 1
  }
  return { fields: out, close }
}

/** Every hidden block id (without the `^`) in the frontmatter, from either key, in order. */
export function hiddenRefs(text: string): string[] {
  const found = fields(text)
  if (!found) return []
  const ids: string[] = []
  for (const f of found.fields) for (const id of f.ids) if (!ids.includes(id)) ids.push(id)
  return ids
}

/** The edits that make the frontmatter's hidden blocks exactly `ids` (none removes the field). */
export function hiddenEdits(text: string, ids: string[]): Edit[] {
  const found = fields(text)
  const line = ids.length ? `${KEY}: [${ids.map((id) => `"^${id}"`).join(', ')}]\n` : ''
  if (!found) return ids.length ? [{ from: 0, to: 0, insert: `---\n${line}---\n` }] : []
  const own = found.fields.filter((f) => f.key === KEY)
  const old = found.fields.filter((f) => f.key === OLD)
  const edits: Edit[] = []
  const first = own[0] ?? old[0]
  for (const f of [...own, ...old]) edits.push({ from: f.from, to: f.to, insert: f === first ? line : '' })
  if (!first && line) edits.push({ from: found.close, to: found.close, insert: line })
  return edits.sort((a, b) => a.from - b.from)
}
