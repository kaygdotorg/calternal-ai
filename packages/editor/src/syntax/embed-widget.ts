/**
 * The widget that takes a line of embeds' place: one atomic piece holding a card, a row or a gallery, with
 * its mini toolbar. Clicks, the keyboard and the context menu all go to the app (see {@link EmbedHandlers});
 * the editor never moves its caret into it.
 */

import { type EditorView, WidgetType } from '@codemirror/view'
import { audioCard, el, fileRow, pictureCard, videoCard } from './embed-dom'
import { gallery } from './embed-gallery'
import { miniBar, noteCard } from './embed-note'
import { type EmbedHandlers, type EmbedItem, isGallery, shownName } from './embeds'

const KIND_WORD = { image: 'Photo', gif: 'GIF', video: 'Video', audio: 'Recording', pdf: 'PDF', file: 'File', note: 'Note' } as const

const same = (a: EmbedItem, b: EmbedItem): boolean =>
  a.href === b.href &&
  a.alt === b.alt &&
  a.layout === b.layout &&
  a.source.src === b.source.src &&
  a.source.size === b.source.size &&
  a.source.w === b.source.w &&
  a.source.h === b.source.h &&
  a.source.pages === b.source.pages &&
  JSON.stringify(a.source.note) === JSON.stringify(b.source.note)

/** The one mini toolbar in the page (see `toDOM`): the embed that has it is the one being pointed at or focused. */
let shared: HTMLElement | null = null

export class EmbedWidget extends WidgetType {
  constructor(
    readonly items: EmbedItem[],
    readonly handlers: EmbedHandlers | null,
    /** An SVG for an icon, by name: set by the app (the editor draws plain elements). */
    readonly icon: (name: string) => string = () => '',
  ) {
    super()
  }

  override eq(o: EmbedWidget): boolean {
    return o.items.length === this.items.length && this.items.every((i, n) => same(i, o.items[n]!))
  }

  override toDOM(view: EditorView): HTMLElement {
    const items = this.items
    const first = items[0]!
    const h = this.handlers
    const icon = this.icon
    const root = el('span', 'cm-embed')
    const event = (index: number) => ({ items, index, el: root })
    const rename = h ? (words: string) => h.rename(event(0), words) : undefined
    let inner: HTMLElement
    // Where the mini toolbar sits: in a card's top corner; at the end of a row; under a gallery.
    let place = 'corner'
    if (isGallery(items)) {
      root.dataset.kind = 'gallery'
      place = 'foot'
      inner = gallery(items, first.layout === 'grid', (n) => h?.open(event(n)), view.contentDOM.clientWidth)
    } else {
      root.dataset.kind = first.kind
      root.dataset.layout = first.layout ?? (first.kind === 'audio' || first.kind === 'file' ? 'compact' : 'card')
      const card = (broken = false): HTMLElement => {
        if (broken) return fileRow(first, 'Photo', false)
        switch (first.kind) {
          case 'image':
          case 'gif':
            root.classList.add('cm-image')
            return pictureCard(
              first,
              () => {
                root.classList.remove('cm-image')
                root.dataset.kind = 'file'
                root.replaceChildren(card(true))
              },
              rename,
            )
          case 'video':
            return videoCard(first, rename)
          case 'audio':
            place = 'inline'
            return audioCard(first)
          case 'pdf':
            return fileRow(first, KIND_WORD.pdf, first.layout !== 'compact')
          case 'note': {
            const compact = first.layout === 'compact' && first.source.note?.state === 'ok'
            if (compact) place = 'inline'
            return noteCard(first, { custom: first.alt.trim(), rename: (w) => rename?.(w), act: (what) => h?.note(event(0), what), icon })
          }
          default:
            place = 'inline'
            return fileRow(first, KIND_WORD.file, false)
        }
      }
      inner = card()
      if (first.kind === 'pdf' && first.layout === 'compact') place = 'inline'
      // Pictures, notes, PDFs and files go to the app on a tap; videos and recordings play where they are.
      if (first.kind !== 'video' && first.kind !== 'audio') {
        // A double click on the name changes it (embed-dom `editable`); the first click of it waits a moment before opening.
        let pending: ReturnType<typeof setTimeout> | undefined
        root.addEventListener('dblclick', () => clearTimeout(pending), true)
        root.addEventListener('click', (ev) => {
          ev.preventDefault()
          clearTimeout(pending)
          if (ev.detail >= 2) return
          if (first.kind === 'note' && first.source.note?.state !== 'ok') return
          if ((ev.target as HTMLElement).closest('.cm-embed-label, .cm-embed-ntitle')) pending = setTimeout(() => h?.open(event(0)), 260)
          else h?.open(event(0))
        })
      }
    }
    root.append(inner)
    if (h) {
      // One toolbar for the whole editor, built for the embed the pointer or the focus is on and moved there: an embed nobody is
      // pointing at carries no toolbar at all, so a key typed elsewhere pays nothing for them.
      const bring = () => {
        const bar = miniBar(
          h.tools(event(0)),
          icon,
          (id) => h.layout(event(0), id),
          (at) => {
            const r = at.getBoundingClientRect()
            h.menu(event(0), r.right, r.bottom + 6)
          },
          place,
        )
        shared?.remove()
        shared = bar
        root.append(bar)
      }
      root.addEventListener('pointerenter', () => !root.contains(shared) && bring())
      root.addEventListener('focusin', () => !root.contains(shared) && bring())
    }
    root.dataset.sig = this.sig()
    root.tabIndex = 0
    root.setAttribute('role', 'group')
    root.setAttribute('aria-label', isGallery(items) ? `Gallery, ${items.length} photos` : `${KIND_WORD[first.kind]}: ${shownName(first)}`)
    root.addEventListener('contextmenu', (ev) => {
      if (!h) return
      ev.preventDefault()
      const at = (ev.target as HTMLElement).closest('.cm-embed-card, .cm-embed-cell')
      const n = at ? [...at.parentElement!.children].indexOf(at) : 0
      h.menu(event(Math.max(0, n)), ev.clientX, ev.clientY)
    })
    root.addEventListener('keydown', (ev) => {
      if (ev.target !== root || !h) return
      if (ev.key === 'Enter' || ev.key === ' ') {
        ev.preventDefault()
        if (first.kind === 'note' && first.source.note?.state !== 'ok') return
        h.open(event(0))
      }
    })
    return root
  }

  /** Something that is playing is not torn down for a change in what is known about the file (its size arriving). */
  override updateDOM(dom: HTMLElement): boolean {
    return dom.hasAttribute('data-playing') && dom.dataset.sig === this.sig()
  }

  /** What is written on the line: a change to that always draws the embed again. */
  private sig(): string {
    return this.items.map((i) => `${i.href}|${i.alt}|${i.layout ?? ''}`).join('\n')
  }

  /** The embed handles its own clicks and keys; the editor never moves its caret into it. */
  override ignoreEvent(): boolean {
    return true
  }
}
