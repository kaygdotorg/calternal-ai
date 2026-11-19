/**
 * The inline picker's code (the plugin, its rows and its keys), fetched when the browser is idle after the editor shows, or by the
 * first trigger typed (picker.ts, the part the first screen carries). Everything below is what that file used to be.
 *
 * The inline picker: one glass list under the caret for `/` (block actions,
 * insert/registry.ts), `[[` and `@` (notes to link, links.ts) and `#` (tags,
 * tags.ts). One list, one set of keys: ↑ ↓ move (round the ends), Enter takes
 * the row, Tab takes it too (a tag with tags inside it opens it instead), Esc
 * closes. It never takes the caret away: you keep typing in the note, and the
 * keys mean something to the list only while it shows.
 *
 * This file decides when it is open and what it holds; the app draws it
 * (a {@link PickerHost}). It closes when the caret leaves what it is
 * completing, on Esc, when the note loses focus (a click anywhere else) and
 * when another note is shown. It appears at once: it comes from typing, and
 * keyboard actions never animate.
 *
 * On the typing path it does nothing while closed, but look at the line's last
 * 120 characters once for a trigger. Pastes, undo and other devices' text never
 * open it.
 */

import { Facet, Prec } from '@codemirror/state'
import { type EditorView, type ViewUpdate, ViewPlugin, keymap } from '@codemirror/view'
import { findActions } from '../insert/find'
import { insertBlockAction } from '../insert/insert'
import { registerDefaultBlocks } from '../insert/plugins'
import { type BlockAction, type PickKind, blockActions } from '../insert/registry'
import { isRemote } from '../origins'
import { type LinkOptions, dayRows, linkRows } from './links'
import { type PickerHost, type PickerShown, askPicker, pickKey } from './picker'
import type { PickRow } from './rows'
import { type TagOptions, inPlainText, tagRows } from './tags'

const WINDOW = 120
// `before` starts with a mark when the window cut the line, so a start-of-line rule never matches mid-line.
const CUT = '\u0001'
/** A `/` opens the menu only at the start of a block or after a space, and its word ends at the next space or `/`: "and/or" and "/usr/bin" stay text. */
const SLASH = /(^|\s)\/([^\s/][^/\n]{0,24})?$/
/** `#` then a letter: "issue #12" and a lone `#` never open it (unless the Insert sheet asked). */
const TAG = /(^|[\s(])#(\p{L}[\p{L}\p{N}_/-]*)?$/u
const NOTE = /\[\[([^[\]\n]*)$|(^|\s)@([^\s@]*)$/

interface Hit {
  kind: PickerShown['kind']
  from: number
  query: string
}

/** What the words just before the caret ask for, or null. */
function detect(view: EditorView, asked: PickKind | null): Hit | null {
  const { state } = view
  const sel = state.selection.main
  if (!sel.empty) return null
  const line = state.doc.lineAt(sel.head)
  // The note's title (its `#` line) and the properties are not places for any of it.
  if (/^#\s/.test(line.text)) return null
  const start = Math.max(0, sel.head - line.from - WINDOW)
  const before = (start > 0 ? CUT : '') + line.text.slice(start, sel.head - line.from)
  const at = (idx: number) => line.from + start + idx - (start > 0 ? 1 : 0)

  const sl = SLASH.exec(before)
  if (sl) {
    const from = at(sl.index + sl[1]!.length)
    if (inPlainText(state, from) && !inTable(view, from)) return { kind: 'slash', from, query: sl[2] ?? '' }
    return null
  }
  const t = TAG.exec(before)
  if (t) {
    const q = t[2] ?? ''
    const from = at(t.index + t[1]!.length)
    if (!q && asked !== 'tag') return null
    return inPlainText(state, from) ? { kind: 'tag', from, query: q } : null
  }
  const n = NOTE.exec(before)
  if (n) {
    const wiki = n[1] != null
    const from = at(n.index + (wiki ? 0 : n[2]!.length))
    return inPlainText(state, from) ? { kind: asked === 'day' ? 'day' : 'note', from, query: wiki ? n[1]! : n[3]! } : null
  }
  return null
}

/** A table cell is one line of words: links, days and tags go in it; blocks are blocks of the note. */
function inTable(view: EditorView, pos: number): boolean {
  return /^\s*\|/.test(view.state.doc.lineAt(pos).text)
}

const actionRow = (a: BlockAction, group: string): PickRow => ({
  key: a.id,
  label: a.label,
  kind: 'action',
  icon: a.icon,
  badge: a.badge,
  hint: a.shortcut,
  group,
  apply: (view, from, to) => {
    // The typed `/words` go first; then the block goes where the caret's line is.
    view.dispatch({ changes: { from, to, insert: '' }, selection: { anchor: from }, userEvent: 'input.complete' })
    insertBlockAction(view, a)
  },
})

const slashRows = (q: string): PickRow[] => findActions(blockActions(), q).flatMap((g) => g.items.map((a) => actionRow(a, g.group)))

interface State {
  kind: PickerShown['kind']
  from: number
  query: string
  rows: PickRow[]
  active: number
}

class Picker {
  s: State | null = null
  private seq = 0
  private last = { left: 0, top: 0, bottom: 0 }
  private gone = false
  /** Set by the Insert sheet and the `/` menu: this kind opens with nothing typed after its trigger. */
  private asked: PickKind | null = null

  private links: LinkOptions | undefined
  private tags: TagOptions | undefined
  private host: PickerHost

  constructor(private view: EditorView) {
    const c = view.state.facet(config)!
    this.links = c.links
    this.tags = c.tags
    this.host = c.host
  }

  update(u: ViewUpdate) {
    if (!this.s && !u.docChanged) return
    if (u.focusChanged && !this.view.hasFocus) return this.close()
    if (this.view.composing) return this.close()
    for (const tr of u.transactions) for (const e of tr.effects) if (e.is(askPicker)) this.asked = e.value
    if (u.docChanged) {
      // Only the person's own typing, a deletion of it, or a row taken (Tab into a tag) look at the text.
      const typed = u.transactions.every((tr) => !tr.docChanged || (!isRemote(tr) && (tr.isUserEvent('input.type') || tr.isUserEvent('input.complete') || tr.isUserEvent('delete.backward'))))
      if (!typed) {
        // A paste, an undo, another device's text, or the app tidying the note: never opens it. When it is
        // already open it stays only if the words it completes are just as they were.
        if (!this.s) return
        const from = u.changes.mapPos(this.s.from, -1)
        const same = detect(this.view, this.asked)
        if (!same || same.kind !== this.s.kind || same.from !== from || same.query !== this.s.query) return this.close()
        // Text above it moved the words it completes: the list follows them.
        if (from !== this.s.from) {
          this.s = { ...this.s, from }
          this.publish()
        }
        return
      }
    }
    const hit = detect(this.view, this.asked)
    if (!hit) return this.close()
    this.show(hit)
  }

  /** Looks at the caret's words now (they were typed before this code was here), taking `asked` as the Insert sheet's request. */
  start(asked?: PickKind) {
    if (asked) this.asked = asked
    const hit = detect(this.view, this.asked)
    if (hit) this.show(hit)
  }

  private show({ kind, from, query }: Hit) {
    const prev = this.s
    if (prev && prev.kind === kind && prev.from === from && prev.query === query) return
    if (kind === 'note' || kind === 'day') {
      // The list is asked, and stays as it was until the answer comes: no flash of nothing between keys.
      const id = ++this.seq
      if (prev) this.s = { ...prev, kind, from, query }
      const links = this.links
      const rows = !links ? Promise.resolve([]) : kind === 'day' ? dayRows(links, query) : linkRows(links, query)
      rows.then(
        (r) => id === this.seq && !this.gone && this.stillThere(from, query) && this.set(kind, from, query, r, prev),
        () => id === this.seq && this.close(),
      )
      return
    }
    this.seq++
    this.set(kind, from, query, kind === 'slash' ? slashRows(query) : this.tags ? tagRows(this.tags, query) : [], prev)
  }

  /** The answer to a search came back: the caret may have left the trigger meanwhile. */
  private stillThere(from: number, query: string): boolean {
    const hit = detect(this.view, this.asked)
    return !!hit && hit.from === from && hit.query === query
  }

  private set(kind: State['kind'], from: number, query: string, rows: PickRow[], prev: State | null) {
    if (!rows.length) return this.close()
    // The row you are on stays when the same words are asked again; a new question starts at the top.
    const active = prev && prev.kind === kind && prev.query === query && prev.active < rows.length ? prev.active : 0
    this.s = { kind, from, query, rows, active }
    this.publish()
  }

  private publish() {
    const s = this.s
    if (!s) return
    this.host.show({
      kind: s.kind,
      query: s.query,
      rows: s.rows,
      active: s.active,
      anchor: s.from,
      at: () => {
        const box = this.view.coordsAtPos(s.from)
        if (box) this.last = { left: box.left, top: box.top, bottom: box.bottom }
        return this.last
      },
      dom: this.view.dom,
      accept: (i, tab) => this.accept(i, tab),
      setActive: (i) => this.active(i),
      close: () => this.close(),
    })
  }

  active(i: number) {
    if (this.s && i !== this.s.active) {
      this.s = { ...this.s, active: i }
      this.publish()
    }
  }

  move(by: 1 | -1) {
    const s = this.s
    if (!s) return
    this.active(pickKey({ key: by === 1 ? 'ArrowDown' : 'ArrowUp' }, s.active, s.rows.length)!.active!)
  }

  accept(i: number, tab = false) {
    const s = this.s
    const row = s?.rows[i]
    if (!s || !row) return this.close()
    const to = this.view.state.selection.main.head
    this.close()
    ;(tab && row.tab ? row.tab : row.apply)(this.view, s.from, to)
  }

  close() {
    this.seq++
    this.asked = null
    if (!this.s) return
    this.s = null
    this.host.hide()
  }

  destroy() {
    this.gone = true
    this.close()
  }
}

/** What the app gave the editor (the notes and tags to offer, and the glass list that draws the rows). */
interface Config {
  links?: LinkOptions
  tags?: TagOptions
  host: PickerHost
}
const config = Facet.define<Config, Config | null>({ combine: (all) => all[0] ?? null })
const plugin = ViewPlugin.fromClass(Picker)

/** Closes the list if it shows (Vim's Esc asks this, through picker.ts). True when it was open. */
export function closeOpen(view: EditorView): boolean {
  const p = view.plugin(plugin)
  if (!p?.s) return false
  p.close()
  return true
}

/** The words just before the caret may already ask for a list (the trigger was typed before this code was in), or the Insert sheet asked for one. */
export function refresh(view: EditorView, asked?: PickKind): void {
  view.plugin(plugin)?.start(asked)
}

/** The plugin, its keys and its settings; picker.ts puts this into the editor once the code is here. */
export function pickerExtension(links: LinkOptions | undefined, tags: TagOptions | undefined, host: PickerHost) {
  registerDefaultBlocks()
  const open = (run: (p: Picker) => void) => (view: EditorView) => {
    const p = view.plugin(plugin)
    if (!p?.s) return false
    run(p)
    return true
  }
  return [
    config.of({ links, tags, host }),
    plugin,
    // The picker's keys are read before the editor's own, only while it shows.
    Prec.highest(
      keymap.of([
        { key: 'ArrowDown', run: open((p) => p.move(1)) },
        { key: 'ArrowUp', run: open((p) => p.move(-1)) },
        { key: 'Enter', run: open((p) => p.accept(p.s!.active)) },
        { key: 'Tab', run: open((p) => p.accept(p.s!.active, true)) },
        { key: 'Escape', run: open((p) => p.close()) },
      ]),
    ),
  ]
}
