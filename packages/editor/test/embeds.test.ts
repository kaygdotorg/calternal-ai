/** Verifies embed replacement by the lazy embed-lines extension without changing stored markdown, and that it follows edits. */
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { EditorState, type Transaction } from '@codemirror/state'
import { type DecorationSet, EditorView } from '@codemirror/view'
import { describe, expect, it } from 'vitest'
import { embedLines, refreshEmbeds } from '../src/syntax/embed-lines'
import { EmbedWidget } from '../src/syntax/embed-widget'
import { embedKind, embedsOfLine, isGallery, shownName, sizeWords } from '../src/syntax/embeds'

const PIC = '![shell](<../Photos/2026/2026-10-17/shell 2.jpg>)'
const known = (href: string) => (/(Photos|Documents)\//.test(href) ? { src: `/a/${href}`, name: href.split('/').pop()!, w: 4, h: 3 } : null)

/** The app's answer: embeds on a line that holds nothing else, for files it knows. `asked` collects the lines it was asked about. */
function widgetFor(on: boolean, asked?: string[]) {
  return (line: string) => {
    asked?.push(line)
    const items = on ? embedsOfLine(line, known) : null
    return items ? new EmbedWidget(items, null) : null
  }
}

function state(doc: string, on = true, asked?: string[]) {
  const s = EditorState.create({ doc, extensions: [markdown({ base: markdownLanguage }), embedLines(widgetFor(on, asked))] })
  ensureSyntaxTree(s, s.doc.length, 5000)
  return s
}

/** The text the caret steps over (the embeds), in document order. */
function stepped(s: EditorState): string[] {
  const out: string[] = []
  const iter = s.facet(EditorView.atomicRanges)[0]!({ state: s, dom: { dataset: {} } } as unknown as EditorView).iter()
  for (; iter.value; iter.next()) out.push(s.doc.sliceString(iter.from, iter.to))
  return out
}

const replaced = (doc: string, on = true): string[] => stepped(state(doc, on))
const typed = (s: EditorState, at: number, text: string): EditorState => s.update({ changes: { from: at, insert: text } }).state

describe('embeds', () => {
  it('draws a picture alone on its line as one piece', () => {
    expect(replaced(`before\n\n${PIC}\n\nafter\n`)).toEqual([PIC])
  })
  it('leaves the file untouched: the text is the line, byte for byte', () => {
    const doc = `a\n\n${PIC}\n${PIC}\n`
    expect(state(doc).doc.toString()).toBe(doc)
    expect(replaced(doc)).toEqual([PIC, PIC])
  })
  it('keeps the markdown as text when words share the line', () => {
    expect(replaced(`look ${PIC} here\n`)).toEqual([])
  })
  it('keeps the markdown as text when the app does not know the file', () => {
    expect(replaced(`${PIC}\n`, false)).toEqual([])
  })
  it('does not draw a web picture or a note embed, nor inside code, a list or a quote', () => {
    expect(replaced('![x](https://example.org/a.png)\n')).toEqual([])
    expect(replaced('![T](<Other.md>)\n')).toEqual([])
    expect(replaced('```\n' + PIC + '\n```\n')).not.toContain(PIC)
    expect(replaced(`- ${PIC}\n`)).not.toContain(`- ${PIC}`)
    expect(replaced(`> ${PIC}\n`)).not.toContain(`> ${PIC}`)
  })
  it('draws files, videos, recordings and a layout word', () => {
    const doc = ['![t](<../Documents/2026/2026-10-17/Tickets.pdf>)', '![v](<../Photos/2026/2026-10-17/clip.mp4> "large")', '![r](<../Documents/2026/2026-10-17/memo.m4a>)'].join('\n\n') + '\n'
    expect(replaced(doc)).toHaveLength(3)
  })
  it('several embeds on one line, separated by spaces, are one gallery', () => {
    const line = `![a](<../Photos/a.jpg> "grid") ![b](<../Photos/b.jpg>) ![c](<../Photos/c.png>)`
    const items = embedsOfLine(line, known)!
    expect(items.map((i) => i.alt)).toEqual(['a', 'b', 'c'])
    expect(items[0]!.layout).toBe('grid')
    expect(isGallery(items)).toBe(true)
    expect(replaced(`${line}\n`)).toEqual([line])
    // Words between them make it text again.
    expect(embedsOfLine(`${line} and`, known)).toBeNull()
  })
  it('a gallery holds pictures only', () => {
    const items = embedsOfLine('![a](<../Photos/a.jpg>) ![t](<../Documents/t.pdf>)', known)!
    expect(isGallery(items)).toBe(false)
  })
  it('knows what a file is from its name', () => {
    expect(embedKind('../Photos/a.HEIC')).toBe('image')
    expect(embedKind('x.gif')).toBe('gif')
    expect(embedKind('x.mov')).toBe('video')
    expect(embedKind('x.opus')).toBe('audio')
    expect(embedKind('x.pdf#page=2')).toBe('pdf')
    expect(embedKind('x.zip')).toBe('file')
  })
  it('says sizes and names in words', () => {
    expect(sizeWords(152_000)).toBe('152 KB')
    expect(sizeWords(1_400_000)).toBe('1.4 MB')
    expect(sizeWords(12_000_000)).toBe('12 MB')
    const [i] = embedsOfLine('![](<../Photos/canal.jpg>)', known)!
    expect(shownName(i!)).toBe('canal')
    const [j] = embedsOfLine('![Along the canal](<../Photos/canal.jpg>)', known)!
    expect(shownName(j!)).toBe('Along the canal')
  })
})

describe('following edits', () => {
  it('keeps an embed where it is when text is typed above it', () => {
    expect(stepped(typed(state(`a\n\n${PIC}\n`), 1, 'bc'))).toEqual([PIC])
  })
  it('asks the app about the changed line only, not about the embeds elsewhere', () => {
    const asked: string[] = []
    const s0 = state(`${PIC}\n${PIC}\ntext\n`, true, asked)
    asked.length = 0
    typed(s0, s0.doc.length - 1, 'x')
    expect(asked).toEqual([])
  })
  it('does no work on the drawn embeds for a key typed after the last one, and still follows one typed before', () => {
    // The facet holds the decoration function itself; its type also allows a plain set, so narrow it here.
    const drawn = (s: EditorState) =>
      (s.facet(EditorView.decorations)[0] as (view: EditorView) => DecorationSet)({ state: s, dom: { dataset: {} } } as unknown as EditorView)
    const s0 = state(`a\n\n${PIC}\n\ntext\n`)
    expect(drawn(typed(s0, s0.doc.length - 1, 'x'))).toBe(drawn(s0))
    const before = typed(s0, 1, 'bc')
    expect(drawn(before)).not.toBe(drawn(s0))
    expect(stepped(before)).toEqual([PIC])
    // A line that becomes an embed after the last one is drawn too, and later keys after it leave it alone.
    const later = typed(s0, s0.doc.length, `\n${PIC}\n`)
    expect(stepped(later)).toEqual([PIC, PIC])
    expect(drawn(typed(later, later.doc.length, 'y'))).toBe(drawn(later))
  })
  it('draws an embed line typed or pasted into a note', () => {
    expect(stepped(typed(state('a\n\n'), 3, PIC))).toEqual([PIC])
  })
  it('turns an embed back into text when its line is edited, and removes it with its line', () => {
    const s = typed(state(`${PIC}\n`), PIC.length, ' more')
    expect(stepped(s)).toEqual([])
    const gone = state(`a\n${PIC}\nb\n`).update({ changes: { from: 2, to: 2 + PIC.length + 1 } }).state
    expect(stepped(gone)).toEqual([])
  })
  it('draws everything again when the app learned something', () => {
    let on = false
    const s0 = EditorState.create({ doc: `${PIC}\n`, extensions: [markdown({ base: markdownLanguage }), embedLines((l) => (on ? widgetFor(true)(l) : null))] })
    expect(stepped(s0)).toEqual([])
    on = true
    const tr: Transaction = s0.update({ effects: refreshEmbeds.of(null) })
    expect(stepped(tr.state)).toEqual([PIC])
  })
})
