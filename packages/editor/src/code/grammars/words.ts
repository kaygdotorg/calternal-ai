/**
 * Shell and SQL, coloured by the mockup's own rules: one pass of regular expressions over the block
 * (comments, strings, numbers, keywords, calls). They have no lezer grammar in the bundle, and a block of
 * either is short; the words and colours are the mockup's.
 */

export type Span = { from: number; to: number; cls: 'tk-kw' | 'tk-str' | 'tk-num' | 'tk-com' | 'tk-fn' }

const KEYWORDS = {
  shell: new Set('if then else fi for do done while case esac function in export local return echo cd sudo'.split(' ')),
  sql: new Set('SELECT FROM WHERE AND OR INSERT INTO VALUES UPDATE SET DELETE CREATE TABLE JOIN LEFT ON GROUP BY ORDER LIMIT AS NULL NOT'.split(' ')),
}
const COMMENT = { shell: '#[^\\n]*', sql: '--[^\\n]*' }

const re = (lang: 'shell' | 'sql') =>
  new RegExp(
    [
      `(?<com>${COMMENT[lang]})`,
      `(?<str>"(?:\\\\.|[^"\\\\\\n])*"|'(?:\\\\.|[^'\\\\\\n])*'|\`(?:\\\\.|[^\`\\\\])*\`)`,
      `(?<num>\\b\\d[\\d_]*(?:\\.\\d+)?\\b)`,
      `(?<word>[A-Za-z_]\\w*)`,
    ].join('|'),
    'g',
  )

const cache: Partial<Record<'shell' | 'sql', RegExp>> = {}

export function spans(code: string, lang: 'shell' | 'sql'): Span[] {
  const out: Span[] = []
  const rx = (cache[lang] ??= re(lang))
  rx.lastIndex = 0
  const kw = KEYWORDS[lang]
  for (let m = rx.exec(code); m; m = rx.exec(code)) {
    const from = m.index
    const to = from + m[0].length
    const g = m.groups!
    if (g.com) out.push({ from, to, cls: 'tk-com' })
    else if (g.str) out.push({ from, to, cls: 'tk-str' })
    else if (g.num) out.push({ from, to, cls: 'tk-num' })
    else if (kw.has(lang === 'sql' ? m[0].toUpperCase() : m[0])) out.push({ from, to, cls: 'tk-kw' })
    else if (code[to] === '(' || code[to] === '!') out.push({ from, to, cls: 'tk-fn' })
  }
  return out
}
