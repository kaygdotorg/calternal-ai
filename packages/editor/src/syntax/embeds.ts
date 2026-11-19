/**
 * Embeds: files drawn in place of their markdown.
 *
 * `![alt](<../Photos/2026/2026-10-17/shell.jpg>)` alone on a line stays in the file; in default mode a
 * picture, a video, a recording or a file takes its place, as one atomic piece (the caret steps over it,
 * Backspace takes it out whole). Several on one line, separated by spaces, are a gallery. A quoted word
 * after the address is the layout, the way the file keeps it: `![alt](<…> "large")`.
 *
 * The editor knows nothing about where files live: the app answers with the address to load and what it knows (size, a picture's width and height, a PDF's pages), so the box has its
 * final shape before one byte arrives and nothing below it moves. A target the app does not know stays
 * text. The look lives in the app's stylesheet (`.cm-embed*`).
 */

/** What a file is, from its name alone, so a note opened in any app shows the same thing. */
export type EmbedKind = 'image' | 'gif' | 'video' | 'audio' | 'pdf' | 'file' | 'note'

export function embedKind(href: string): EmbedKind {
  const path = (href.split('#')[0] ?? '').toLowerCase()
  if (path.endsWith('.md')) return 'note'
  if (/\.(jpe?g|png|webp|avif|heic|heif|bmp|tiff?)$/.test(path)) return 'image'
  if (path.endsWith('.gif')) return 'gif'
  if (/\.(mp4|m4v|mov|webm)$/.test(path)) return 'video'
  if (/\.(m4a|mp3|wav|ogg|opus|aac|flac)$/.test(path)) return 'audio'
  if (path.endsWith('.pdf')) return 'pdf'
  return 'file'
}

/** A note shown as a card: what the app knows about the note an embed points at. */
export interface NoteCard {
  /** `ok`: it is there. `new`: never written. `trash`: in the Trash (its lines still show). `gone`: deleted for good. */
  state: 'ok' | 'new' | 'trash' | 'gone'
  /** The note's title, or the name the link asked for when there is no note. */
  title: string
  /** The heading a `Note.md#Heading` embed shows only. */
  part?: string
  /** The part is not in the note any more: the whole note shows. */
  lost?: boolean
  /** The markdown lines the card shows from. */
  lines: string[]
  /** For a note's compact row: its tags. */
  tags: string[]
  /** For `gone`: when it was deleted, in words. */
  deleted?: string
}

/** What the app knows about an embed's target, as written in the note. */
export interface EmbedSource {
  /** Where to load it from. */
  src: string
  /** Its file name (the words shown when the note gave none). */
  name: string
  /** Bytes, when known. */
  size?: number
  /** Pixels across and down, when known: the box keeps this shape from the start. */
  w?: number
  h?: number
  /** A PDF's page count. */
  pages?: number
  /** For a note: the card's content. */
  note?: NoteCard
  /** Called once a picture has loaded with its real size, when the size above was not known. */
  learned?: (w: number, h: number) => void
}

/** One embed on a line, as written. */
export interface EmbedItem {
  alt: string
  href: string
  /** The quoted word after the address, if any. */
  layout?: string
  kind: EmbedKind
  source: EmbedSource
}

/** What a tap on an embed asks the app to do. `el` is the embed's own element. */
export interface EmbedEvent {
  items: EmbedItem[]
  /** Which of `items` was touched (a gallery has several). */
  index: number
  el: HTMLElement
}

/** The mini toolbar on an embed: its layouts as a small segmented choice, the things people do most, then ⋯. */
export interface EmbedTools {
  layouts: { id: string; label: string; icon: string; on: boolean }[]
  actions: { label: string; icon: string; run: () => void }[]
}

export interface EmbedHandlers {
  open(e: EmbedEvent): void
  /** A right-click, a long press or the ⋯ button, at screen point (x, y). */
  menu(e: EmbedEvent, x: number, y: number): void
  /** What the mini toolbar holds. */
  tools(e: EmbedEvent): EmbedTools
  /** Chooses a layout (a width or a shape): the app writes it into the line. */
  layout(e: EmbedEvent, id: string): void
  /** The words shown on a picture or a card, changed by the person (the alt text of the link). */
  rename(e: EmbedEvent, words: string): void
  /** A note card's own buttons. */
  note(e: EmbedEvent, what: 'open' | 'restore' | 'create' | 'remove'): void
}

const ONE = String.raw`!\[([^\]\n]*)\]\(<([^>\n]+)>(?: "([^"\n]*)")?\)`
const ONE_G = new RegExp(ONE, 'g')
const ONLY = new RegExp(String.raw`^(?:${ONE})(?: +(?:${ONE}))*$`)

/** The embeds on a line that holds nothing else, or null. Targets the app does not know give null. */
export function embedsOfLine(text: string, source: (href: string) => EmbedSource | null): EmbedItem[] | null {
  const line = text.trim()
  if (!line.startsWith('![') || !ONLY.test(line)) return null
  const items: EmbedItem[] = []
  for (const m of line.matchAll(ONE_G)) {
    const href = m[2]!
    const s = source(href)
    if (!s) return null
    items.push({ alt: m[1] ?? '', href, layout: m[3] || undefined, kind: embedKind(href), source: s })
  }
  return items.length ? items : null
}

/** A gallery holds two or more pictures. */
export const isGallery = (items: EmbedItem[]): boolean =>
  items.length > 1 && items.every((i) => i.kind === 'image' || i.kind === 'gif')

/** "152 KB", "1.4 MB": the size in words people read. */
export function sizeWords(n: number): string {
  if (n < 1000) return `${n} B`
  if (n < 1e6) return `${Math.round(n / 1000)} KB`
  if (n < 1e9) return `${(n / 1e6).toFixed(n < 1e7 ? 1 : 0).replace(/\.0$/, '')} MB`
  return `${(n / 1e9).toFixed(1)} GB`
}

/** The words shown for an embed: what the note gave, else its file name without the extension. */
export const shownName = (i: EmbedItem): string => i.alt.trim() || i.source.name.replace(/\.[^./]+$/, '')
