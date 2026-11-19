/**
 * The note's Markdown language, with one change: a table's cells are not parsed for bold, links and the like.
 *
 * Why: a table is one block to the Markdown parser, so any edit inside it makes the parser read the whole table
 * again, and the stock table parser also parses the inline Markdown of every cell each time. With 120 rows that was
 * most of the work behind each key typed in a cell. The grid draws a cell's Markdown itself (inline.ts), and nothing
 * else reads inline nodes inside a table (the table is drawn in place of its lines), so the cells carry no inline
 * nodes. The parser still walks the table's lines (a block cannot be skipped), but makes no per-cell work: header
 * and delimiter lines are counted once, later rows are a single `TableRow` node each.
 *
 * Everything else about where a table starts and ends is the stock parser's own (its `endLeaf` rule and its Table
 * node), so a table covers the same lines it always did. `tables-parser.test.ts` checks that against the stock parser.
 */

import { markdownLanguage } from '@codemirror/lang-markdown'
import { Language } from '@codemirror/language'

/** The pieces of the Markdown parser this file touches: the block parser's `leaf` hooks, internal but stable at the pinned version. */
interface Inner {
  blockNames: string[]
  leafBlockParsers: (((cx: unknown, leaf: { content: string }) => object | null) | undefined)[]
}

const delimiterLine = /^[>\s]*\|?(\s*:?-+:?\s*\|)+(\s*:?-+:?\s*)?$/

/** Counts the cells of a pipe line (an escaped pipe does not split), the way the stock parser does, without building anything. */
export function countCells(line: string, start = 0): number {
  let count = 0
  let first = true
  let cellStart = -1
  let esc = false
  for (let i = start; i < line.length; i++) {
    const next = line.charCodeAt(i)
    if (next === 124 && !esc) {
      if (!first || cellStart > -1) count++
      first = false
      cellStart = -1
    } else if (esc || (next !== 32 && next !== 9)) {
      if (cellStart < 0) cellStart = i
    }
    esc = !esc && next === 92
  }
  if (cellStart > -1) count++
  return count
}

const hasPipe = (text: string): boolean => {
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i)
    if (c === 124) return true
    if (c === 92) i++
  }
  return false
}

/** Builds the note language from the stock Markdown language by swapping the table block's line handling. */
function build(): Language {
  const parser = markdownLanguage.parser as unknown as Inner & { configure(spec: unknown): typeof markdownLanguage.parser }
  // The stock leaf hook makes a stock table parser; its class is the base of ours, so the stock `endLeaf` rule still recognises it.
  const Stock = parser.leafBlockParsers[parser.blockNames.indexOf('Table')]!({}, { content: '|' })!.constructor as unknown as new () => {
    rows: unknown[] | false | null
  }

  /** The stock table parser's line handling, minus the per-cell inline parse (`finish` is the stock one). */
  class LightTableParser extends Stock {
    nextLine(
      cx: { elt(type: string, from: number, to: number): unknown; lineStart: number },
      line: { next: number; text: string; pos: number },
      leaf: { content: string; start: number },
    ): boolean {
      if (this.rows == null) {
        // The second line: a table only if it is a delimiter row with as many cells as the header.
        this.rows = false
        const text = line.text.slice(line.pos)
        if ((line.next === 45 || line.next === 58 || line.next === 124) && delimiterLine.test(text) && countCells(leaf.content) === countCells(text))
          this.rows = [
            cx.elt('TableHeader', leaf.start, leaf.start + leaf.content.length),
            cx.elt('TableDelimiter', cx.lineStart + line.pos, cx.lineStart + line.text.length),
          ]
      } else if (this.rows) this.rows.push(cx.elt('TableRow', cx.lineStart + line.pos, cx.lineStart + line.text.length))
      return false
    }
  }

  const leaf = (_cx: unknown, l: { content: string }) => (hasPipe(l.content) ? new LightTableParser() : null)
  return new Language(markdownLanguage.data, parser.configure({ parseBlock: [{ name: 'Table', leaf }] }) as never, [], 'markdown')
}

/** The Markdown language the note editor uses. */
export const noteMarkdown: Language = build()
