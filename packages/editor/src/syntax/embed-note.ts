/**
 * A note embedded as a live card (`![Dal makhani](<Dal makhani.md>)`, or one part: `<Dal makhani.md#Ingredients>`),
 * the embed's mini toolbar, and the words on a card that the person can change (the link's alt text).
 *
 * The card is read only and stays live: the app hands the editor the note's lines whenever the note
 * changes, and the card is drawn again. Tapping it opens the note. A note that was never written, is in
 * the Trash or was deleted for good says so in plain words, with what can be done about it.
 */

import { editable, el, lift } from './embed-dom'
import type { EmbedItem, EmbedTools, NoteCard } from './embeds'

const SHOWN = 6

/** Inline markdown as elements: bold, italic, strike, highlight, code and links (their words). */
function inline(text: string, into: HTMLElement): void {
  const re = /(\*\*[^*]+\*\*|__[^_]+__|~~[^~]+~~|==[^=]+==|`[^`]+`|\*[^*\s][^*]*\*|_[^_\s][^_]*_|!?\[[^\]]*\]\((?:<[^>]*>|[^)]*)\))/g
  let at = 0
  for (const m of text.matchAll(re)) {
    if (m.index > at) into.append(text.slice(at, m.index))
    const t = m[0]
    let node: HTMLElement
    if (t.startsWith('![')) node = el('span', '')
    else if (t.startsWith('[')) node = el('span', 'cm-embed-nlink', /^\[([^\]]*)\]/.exec(t)?.[1] ?? '')
    else if (t.startsWith('**') || t.startsWith('__')) node = el('strong', '', t.slice(2, -2))
    else if (t.startsWith('~~')) node = el('s', '', t.slice(2, -2))
    else if (t.startsWith('==')) node = el('mark', '', t.slice(2, -2))
    else if (t.startsWith('`')) node = el('code', '', t.slice(1, -1))
    else node = el('em', '', t.slice(1, -1))
    into.append(node)
    at = m.index + t.length
  }
  if (at < text.length) into.append(text.slice(at))
}

/** One line of the note, as a block the card can show. Blank lines are spacing, not blocks. */
function block(raw: string): HTMLElement | null {
  const line = raw.replace(/\s\^[a-z0-9-]{3,}$/i, '')
  if (!line.trim()) return null
  const row = el('div', 'cm-embed-nblock')
  let m: RegExpExecArray | null
  if ((m = /^(#{1,6}) +(.*)$/.exec(line))) {
    row.classList.add('cm-embed-nh')
    inline(m[2]!, row)
  } else if ((m = /^(\s*)[-*+] \[([ xX/-])\] (.*)$/.exec(line))) {
    const done = m[2] === 'x' || m[2] === 'X'
    row.classList.add('cm-embed-ntodo')
    if (done) row.classList.add('done')
    row.style.setProperty('--indent', String(Math.floor(m[1]!.length / 2)))
    row.append(el('span', 'cm-embed-nbox', done ? '✓' : ''))
    inline(m[3]!, row.appendChild(el('span', '')))
  } else if ((m = /^(\s*)[-*+] (.*)$/.exec(line))) {
    row.classList.add('cm-embed-nli')
    row.style.setProperty('--indent', String(Math.floor(m[1]!.length / 2)))
    row.append(el('span', 'cm-embed-nbullet', '•'))
    inline(m[2]!, row.appendChild(el('span', '')))
  } else if ((m = /^(\s*)(\d+)[.)] (.*)$/.exec(line))) {
    row.classList.add('cm-embed-nli')
    row.style.setProperty('--indent', String(Math.floor(m[1]!.length / 2)))
    row.append(el('span', 'cm-embed-nbullet', `${m[2]}.`))
    inline(m[3]!, row.appendChild(el('span', '')))
  } else if ((m = /^> ?(.*)$/.exec(line))) {
    row.classList.add('cm-embed-nquote')
    inline(m[1]!, row)
  } else if (/^---+$/.test(line.trim())) {
    row.classList.add('cm-embed-nrule')
  } else inline(line.trim(), row)
  return row
}

/** The mini toolbar: layouts as a segmented choice, a few actions, then ⋯ for the whole menu. */
export function miniBar(tools: EmbedTools, icon: (name: string) => string, onLayout: (id: string) => void, onMore: (at: HTMLElement) => void, place = 'corner'): HTMLElement {
  const bar = el('span', 'cm-embed-minibar pillbar')
  bar.dataset.place = place
  bar.setAttribute('role', 'toolbar')
  bar.setAttribute('aria-label', 'Embed tools')
  bar.addEventListener('click', (e) => e.stopPropagation())
  bar.addEventListener('dblclick', (e) => e.stopPropagation())
  const button = (label: string, svg: string, extra?: (b: HTMLButtonElement) => void): HTMLButtonElement => {
    const b = el('button', '')
    b.type = 'button'
    b.setAttribute('aria-label', label)
    b.title = label
    b.innerHTML = svg
    extra?.(b)
    return b
  }
  if (tools.layouts.length > 1) {
    const seg = el('span', 'cm-embed-seg')
    seg.setAttribute('role', 'radiogroup')
    seg.setAttribute('aria-label', 'Show as')
    for (const l of tools.layouts) {
      seg.append(
        button(l.label, icon(l.icon), (b) => {
          b.setAttribute('role', 'radio')
          b.setAttribute('aria-checked', String(l.on))
          b.classList.toggle('on', l.on)
          b.addEventListener('click', () => onLayout(l.id))
        }),
      )
    }
    bar.append(seg, el('span', 'cm-embed-rule'))
  }
  for (const a of tools.actions) bar.append(button(a.label, icon(a.icon), (b) => b.addEventListener('click', a.run)))
  bar.append(button('More', icon('more'), (b) => (b.setAttribute('aria-haspopup', 'menu'), b.addEventListener('click', () => onMore(b)))))
  return bar
}

/** The note card, or its compact row. */
export function noteCard(i: EmbedItem, ctx: { custom: string; rename: (words: string) => void; act: (what: 'open' | 'restore' | 'create' | 'remove') => void; icon: (name: string) => string }): HTMLElement {
  const n: NoteCard = i.source.note!
  const compact = i.layout === 'compact' && n.state === 'ok'
  if (compact) {
    const row = el('span', 'cm-embed-file cm-embed-nrow')
    const tile = el('span', 'cm-embed-tile')
    tile.innerHTML = ctx.icon('notes')
    const names = el('span', 'cm-embed-names')
    names.append(el('span', 'cm-embed-name', ctx.custom || n.title), el('span', 'cm-embed-meta', `Note · ${n.tags.map((t) => `#${t}`).join(' ') || 'no tags'}`))
    row.append(tile, names)
    return lift(row)
  }
  const card = el('span', 'cm-embed-note')
  card.dataset.state = n.state
  const words =
    n.state === 'trash' ? `“${n.title}” is in the Trash` : n.state === 'gone' ? `“${n.title}” was deleted${n.deleted ? ` on ${n.deleted}` : ''}` : n.state === 'new' ? `“${n.title}” isn't written yet` : ctx.custom || n.title
  const head = el('span', 'cm-embed-nhead')
  const title = el('span', 'cm-embed-ntitle', words)
  head.append(title)
  if (n.state === 'ok') editable(title, ctx.custom || n.title, !!ctx.custom, ctx.rename, 'Double-click to change the title')
  card.append(head)
  if (n.state === 'ok' && n.part) {
    card.append(el('span', 'cm-embed-npart', n.lost ? "That part isn't in this note any more. Showing the note." : n.part))
  }
  let expanded = false
  const foot = el('span', 'cm-embed-nfoot')
  if (n.state === 'ok' || (n.state === 'trash' && n.lines.length)) {
    const body = el('span', 'cm-embed-nbody')
    // Blank lines are spacing, not lines: six lines are six blocks.
    const blocks = n.lines.map(block).filter((b): b is HTMLElement => !!b)
    const shown = (): void => {
      body.replaceChildren(...blocks.slice(0, expanded ? undefined : SHOWN))
      body.classList.toggle('fade', !expanded && blocks.length > SHOWN)
      body.classList.toggle('dim', n.state === 'trash')
    }
    shown()
    card.append(body)
    if (n.state === 'ok' && blocks.length > SHOWN) {
      const more = el('button', 'cm-embed-act link', `Show all ${blocks.length} lines`)
      more.type = 'button'
      more.addEventListener('click', (e) => {
        e.stopPropagation()
        expanded = !expanded
        more.textContent = expanded ? 'Show less' : `Show all ${blocks.length} lines`
        shown()
      })
      foot.append(more)
    }
  }
  const act = (label: string, what: 'open' | 'restore' | 'create' | 'remove', primary = false): HTMLButtonElement => {
    const b = el('button', primary ? 'cm-embed-act primary' : 'cm-embed-act', label)
    b.type = 'button'
    b.addEventListener('click', (e) => (e.stopPropagation(), ctx.act(what)))
    return b
  }
  if (n.state === 'ok') {
    const open = act('Open', 'open')
    open.classList.add('open')
    open.setAttribute('aria-label', `Open ${n.title}`)
    open.insertAdjacentHTML('beforeend', ctx.icon('arrow'))
    foot.append(open)
  } else if (n.state === 'trash') foot.append(act('Restore', 'restore', true), act('Remove embed', 'remove'))
  else if (n.state === 'gone') foot.append(act('Create a new note', 'create', true), act('Remove embed', 'remove'))
  else foot.append(act('Make the note', 'create', true), act('Remove embed', 'remove'))
  card.append(foot)
  return lift(card)
}
