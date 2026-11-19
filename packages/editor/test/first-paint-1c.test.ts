/**
 * Plan 1c, the editor's first paint: opening a long note parses only what is drawn, and the parts built from the
 * syntax tree (embeds, tables, code colours, the page's heading pills) follow the tree as it grows, ending up
 * exactly where a fully parsed note would have them. The text itself is never touched.
 */
import { markdownLanguage } from '@codemirror/lang-markdown'
import { LanguageSupport, ensureSyntaxTree } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { type DecorationSet, EditorView } from '@codemirror/view'
import { describe, expect, it } from 'vitest'
import { codeBlockParts } from '../src/code/blocks'
import { headingsOf, parsedTo } from '../src/syntax/headings'
import { embedLines } from '../src/syntax/embed-lines'
import { EmbedWidget } from '../src/syntax/embed-widget'
import { embedsOfLine } from '../src/syntax/embeds'
import { parseAhead } from '../src/syntax/parse-ahead'
import { tableRanges, tables } from '../src/tables/tables'

const lang = new LanguageSupport(markdownLanguage)
const PIC = '![shell](<../Photos/2026/shell.jpg>)'
const known = (href: string) => (/Photos\//.test(href) ? { src: `/a/${href}`, name: 'shell.jpg', w: 4, h: 3 } : null)
const widgetFor = (line: string) => {
  const items = embedsOfLine(line, known)
  return items ? new EmbedWidget(items, null) : null
}

/** A long note: prose sections with a heading, a shell code block and a table each; `tail` goes at the very end. */
function longNote(sections: number, tail = ''): string {
  const out: string[] = []
  for (let s = 0; s < sections; s++)
    out.push(
      `## Section ${s}`,
      '',
      `Words in section ${s}, **strong** and a [link](<Note ${s}.md>), long enough to wrap a little on a narrow page.`,
      '',
      '```shell',
      `echo "step ${s}"`,
      'ls -la ~/Pictures',
      '```',
      '',
      '| Name | Count |',
      '| --- | --- |',
      `| row | ${s} |`,
      '',
    )
  return out.join('\n') + tail
}

/** Gives the state the parser's tree up to `upto`, as the editor does when it parses further (an empty transaction). */
function parseTo(state: EditorState, upto: number): EditorState {
  ensureSyntaxTree(state, upto, 10_000)
  return state.update({}).state
}

/** Every step of the way, in `steps` slices, as idle parsing does. */
function parseInSteps(state: EditorState, steps: number): EditorState {
  const len = state.doc.length
  for (let i = 1; i <= steps; i++) state = parseTo(state, Math.ceil((len * i) / steps))
  return state
}

/** The ranges of a decoration set with their classes or widget kinds, in order. */
function ranges(set: DecorationSet): string[] {
  const out: string[] = []
  for (const c = set.iter(); c.value; c.next()) out.push(`${c.from}-${c.to}:${c.value.spec.class ?? c.value.spec.widget?.constructor.name ?? ''}`)
  return out
}

/** All decorations a state's fields provide (not depending on a view), as plain ranges. */
function drawn(state: EditorState): string[] {
  const view = { state, dom: { dataset: {} } } as unknown as EditorView
  return state
    .facet(EditorView.decorations)
    .flatMap((d) => ranges(typeof d === 'function' ? d(view) : d))
    .sort()
}

const EMBED_FAR = longNote(1500, `\n${PIC}\n\n\`\`\`\n${PIC}\n\`\`\`\n`)

describe('opening a long note parses only the start', () => {
  it('builds the embed, table and code parts without parsing the whole note', () => {
    const state = EditorState.create({ doc: EMBED_FAR, extensions: [lang, embedLines(widgetFor), tables, codeBlockParts] })
    expect(state.doc.length).toBeGreaterThan(200_000)
    expect(parsedTo(state)).toBeLessThan(state.doc.length / 4)
    expect(state.doc.toString()).toBe(EMBED_FAR)
  })

  it('the page pills read the headings parsed so far, and more as the tree grows', () => {
    let state = EditorState.create({ doc: EMBED_FAR, extensions: [lang] })
    const first = headingsOf(state, false)
    expect(parsedTo(state)).toBeLessThan(state.doc.length)
    expect(first.length).toBeGreaterThan(0)
    expect(first.length).toBeLessThan(1500)
    state = parseTo(state, state.doc.length)
    expect(headingsOf(state, false)).toEqual(headingsOf(state))
    expect(headingsOf(state)).toHaveLength(1500)
  })
})

describe('the parts follow the tree to the same end', () => {
  const extensions = [lang, embedLines(widgetFor), tables, codeBlockParts]

  it('draws the far embed once the tree reaches it, and never the one in code', () => {
    let state = EditorState.create({ doc: EMBED_FAR, extensions })
    const at = EMBED_FAR.indexOf(PIC)
    expect(drawn(state).some((r) => r.startsWith(`${at}-`))).toBe(false)
    state = parseInSteps(state, 7)
    const embeds = drawn(state).filter((r) => r.endsWith(':EmbedWidget'))
    expect(embeds).toEqual([`${at}-${at + PIC.length}:EmbedWidget`])
  })

  it('ends up with the same decorations in one step or in many', () => {
    const once = parseTo(EditorState.create({ doc: EMBED_FAR, extensions }), EMBED_FAR.length)
    const many = parseInSteps(EditorState.create({ doc: EMBED_FAR, extensions }), 13)
    expect(drawn(many)).toEqual(drawn(once))
    // Every shell block was coloured and fenced, the tables were all found.
    expect(drawn(once).filter((r) => r.endsWith(':CodeLangWidget'))).toHaveLength(1501)
    expect(tableRanges(once)).toHaveLength(1500)
  })

  it('an edit past the parsed part is drawn once the tree reaches it, like any other line', () => {
    let state = EditorState.create({ doc: EMBED_FAR, extensions })
    const end = state.doc.length
    state = state.update({ changes: { from: end, insert: `\n${PIC}\n` } }).state
    state = parseInSteps(state, 5)
    const text = state.doc.toString()
    const want = [text.indexOf(PIC), text.lastIndexOf(PIC)].map((at) => `${at}-${at + PIC.length}:EmbedWidget`)
    expect(drawn(state).filter((r) => r.endsWith(':EmbedWidget'))).toEqual(want)
  })

  it('a table is only ever drawn whole, however the parse was cut', () => {
    const rows = Array.from({ length: 50 }, (_, i) => `| ${i} | x |`).join('\n')
    const doc = longNote(400) + '\n| A | B |\n| --- | --- |\n' + rows + '\n\nAfter the table.\n\n' + longNote(50)
    let state = EditorState.create({ doc, extensions: [lang, tables] })
    const start = doc.lastIndexOf('| A | B |')
    const end = start + '| A | B |\n| --- | --- |\n'.length + rows.length
    expect(tableRanges(state).some((r) => r.from === start)).toBe(false)
    // Asked to stop inside the table: it waits, or it is there with every row; never a part of it.
    for (const cut of [start + 30, start + 60, start + 200, end - 5]) {
      const at = parseTo(state, cut)
      const drawn = tableRanges(at).filter((r) => r.from === start)
      expect(drawn.every((r) => r.to === end)).toBe(true)
    }
    state = parseTo(state, doc.length)
    expect(tableRanges(state).find((r) => r.from === start)).toEqual({ from: start, to: end })
  })
})

describe('the parse schedule in a view', () => {
  it('reaches the drawn lines before the frame and the rest of the note at idle, text untouched', async () => {
    const parent = document.createElement('div')
    document.body.append(parent)
    const view = new EditorView({ parent, state: EditorState.create({ doc: EMBED_FAR, extensions: [lang, parseAhead] }) })
    try {
      await Promise.resolve()
      expect(parsedTo(view.state)).toBeGreaterThanOrEqual(view.viewport.to)
      for (let i = 0; i < 200 && parsedTo(view.state) < view.state.doc.length; i++) await new Promise((r) => setTimeout(r, 50))
      expect(parsedTo(view.state)).toBe(view.state.doc.length)
      expect(view.state.doc.toString()).toBe(EMBED_FAR)
    } finally {
      view.destroy()
      parent.remove()
    }
  }, 20_000)
})
