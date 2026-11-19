/**
 * Links drawn as words with a small icon.
 *
 * Every link's visible words get one mark: `cm-link` plus its kind, which
 * says what it points at and picks its icon: `cm-link-note`, `cm-link-day`
 * (a daily note), `cm-link-photo`, `cm-link-file` or `cm-link-web`. The
 * link the caret is on also gets `cm-link-on`, so it can light up. The look
 * lives in the app's stylesheet; this file only names the parts.
 *
 * A link can also carry a state the app knows and the text doesn't, such
 * as "no note lives there yet" or "its note is in the Trash" (#448): the
 * app gives one with the {@link linkState} facet, and it becomes
 * `cm-link-<state>` plus `data-state`. Asking costs nothing per keystroke:
 * the app answers from what it already holds, and tells the editor when an
 * answer changed with {@link refreshLinks}.
 *
 * Each side also says what stands next to the link (#469), so the look can give the pill room
 * without adding a gap: `cm-link-sp-l` / `cm-link-sp-r` when a space is there (the pill's edge
 * can sit inside that space), `cm-link-tight-l` / `cm-link-tight-r` for anything else (a comma,
 * a bracket, a tag or another link touching, the start or end of a line).
 *
 * The mark is never a widget: the words stay ordinary text, so the caret
 * moves through them like any other text, and IME works inside.
 *
 * Link marks are drawn with the highest precedence, so they wrap any other
 * mark inside a link (bold words, a search match) instead of being cut
 * into pieces, and each link stays one piece.
 */

import { type EditorState, Facet, type Range, StateEffect, type Text } from '@codemirror/state'
import { Decoration, type DecorationSet } from '@codemirror/view'

/** What a link points at. */
export type LinkKind = 'note' | 'day' | 'photo' | 'file' | 'web'

/** One link in the text: all of it (`from`–`to`) and its visible words (`textFrom`–`textTo`). */
export interface LinkSpan {
  from: number
  to: number
  textFrom: number
  textTo: number
  href: string
  kind: LinkKind
}

const PHOTO = /\.(png|jpe?g|gif|webp|avif|heic|heif|svg|bmp|tiff?)$/i
/** Daily, weekly, monthly and yearly notes are named by their date. */
const DAY = /(^|\/)(\d{4}(-\d{2}(-\d{2})?|-W\d{2})?)\.md$/i

/**
 * What a link points at, from its target as written: a web address has a
 * scheme (`https:`, `mailto:`); a `.md` file is a note, or a day when it
 * is named by its date; a picture is a photo; anything else is a file.
 */
export function linkKind(href: string): LinkKind {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href)) return 'web'
  const path = href.split('#')[0] ?? ''
  if (PHOTO.test(path)) return 'photo'
  if (/\.md$/i.test(path)) return DAY.test(path) ? 'day' : 'note'
  return path ? 'file' : 'note'
}

/**
 * The app's word for a link's state, from its target as written, or null
 * for a plain link. Cheap: it is asked for every drawn link whenever the
 * links are drawn again.
 */
export const linkState = Facet.define<(href: string) => string | null, (href: string) => string | null>({
  combine: (fs) => (href) => {
    for (const f of fs) {
      const s = f(href)
      if (s) return s
    }
    return null
  },
})

/** Dispatch this when the app's answer for some link changed: the links are drawn again. */
export const refreshLinks = StateEffect.define<null>()

/**
 * The link the caret is on, as an index into `links` (-1 for none). Only a
 * plain caret in a focused editor counts: a stray click elsewhere or a
 * selection across words never lights a link.
 */
export function focusedLink(links: readonly LinkSpan[], state: EditorState, hasFocus: boolean): number {
  if (!hasFocus) return -1
  const sel = state.selection.main
  if (!sel.empty) return -1
  return linkIndexAt(links, sel.head)
}

/** The link whose words hold `pos` (ends included), as an index into `links`, or -1. */
export function linkIndexAt(links: readonly LinkSpan[], pos: number): number {
  // Links are in document order: a binary search keeps this cheap with many links.
  let lo = 0
  let hi = links.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const l = links[mid]!
    if (pos < l.textFrom) hi = mid - 1
    else if (pos > l.textTo) lo = mid + 1
    else return mid
  }
  return -1
}

/** Markup the editor hides around words (bold, strike, highlight): it is not what stands beside a link. */
const HIDDEN_MARKS = new Set(['*', '~', '='])

/**
 * What stands beside a link on one side: `sp` when it is a space (or a tab), `tight` for
 * everything else, including the edge of a line. `from` and `to` are the link's whole span.
 */
export function linkSide(doc: Text, links: readonly LinkSpan[], i: number, side: 'l' | 'r'): 'sp' | 'tight' {
  const l = links[i]!
  // Another link right against this one: its own syntax stands between, and it is a touching glyph.
  if (side === 'l' ? links[i - 1]?.to === l.from : links[i + 1]?.from === l.to) return 'tight'
  let pos = side === 'l' ? l.from : l.to
  for (let n = 0; n < 4; n++) {
    if (side === 'l' ? pos <= 0 : pos >= doc.length) return 'tight'
    const c = side === 'l' ? doc.sliceString(pos - 1, pos) : doc.sliceString(pos, pos + 1)
    if (HIDDEN_MARKS.has(c)) pos += side === 'l' ? -1 : 1
    else return c === ' ' || c === '\t' || c === '\u00a0' ? 'sp' : 'tight'
  }
  return 'tight'
}

/** The marks for `links`, with link number `on` lit and each link's state from `stateOf`. */
export function linkMarks(links: readonly LinkSpan[], on: number, stateOf: (href: string) => string | null, doc: Text): DecorationSet {
  const out: Range<Decoration>[] = []
  links.forEach((l, i) => {
    if (l.textTo <= l.textFrom) return
    const state = l.kind === 'web' ? null : stateOf(l.href)
    const sides = ` cm-link-${linkSide(doc, links, i, 'l')}-l cm-link-${linkSide(doc, links, i, 'r')}-r`
    const cls = `cm-link cm-link-${l.kind}${state ? ` cm-link-${state}` : ''}${i === on ? ' cm-link-on' : ''}${sides}`
    const attributes: Record<string, string> = { 'data-href': l.href }
    if (state) attributes['data-state'] = state
    out.push(Decoration.mark({ class: cls, attributes }).range(l.textFrom, l.textTo))
  })
  return Decoration.set(out, true)
}
