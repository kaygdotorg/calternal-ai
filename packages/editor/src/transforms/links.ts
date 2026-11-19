/**
 * Typing shortcuts for links. `[[` and `@` open a note picker (the list
 * itself is picker.ts); finishing `[[Title]]` by hand also works. Either
 * way, what lands in the file is a standard markdown link, never a
 * wikilink.
 *
 * These run only on the person's own typing: they are an input handler and
 * a completion source, both of which only ever see local keystrokes.
 */

import type { Extension } from '@codemirror/state'
import { EditorView } from '@codemirror/view'
import { markdownLink, relativePath } from './paths'
import type { PickRow } from './rows'

export interface LinkTarget {
  title: string
  /** Home path of the note, e.g. `Notes/Trip ideas.md`. */
  path: string
  /** What the row is (picker rows, see link-rank.ts); a plain note when left out. */
  kind?: 'note' | 'missing' | 'create' | 'mention'
  /** The note's id, when it is one. */
  id?: string
  /** A day note (its row wears the day icon). */
  daily?: boolean
  /** `missing`: how many notes link to it. */
  count?: number
  /** `mention`: where the typed words are, with their ranges (UTF-16) to highlight. */
  snippet?: { text: string; marks: [number, number][] }
}

export interface LinkOptions {
  /** Home path of the note being edited. */
  notePath: () => string
  /** Notes matching what was typed after `[[` or `@`. */
  search?: (query: string) => Promise<LinkTarget[]> | LinkTarget[]
  /** The days the Day link action offers (newest first), for what was typed. */
  days?: (query: string) => Promise<LinkTarget[]> | LinkTarget[]
  /** The note a typed `[[Title]]` points to; defaults to a sibling `Title.md`. */
  resolve?: (title: string) => LinkTarget | null
  /** A row was chosen (so the app can remember what gets linked). */
  chosen?: (target: LinkTarget) => void
}

const CLOSED = /\[\[([^[\]\n]+)\]$/

function linkFor(opts: LinkOptions, target: LinkTarget): string {
  return markdownLink(target.title, relativePath(opts.notePath(), target.path))
}

/** Finishing `[[Title]]` by hand turns it into a link. */
export function closeHandler(opts: LinkOptions): Extension {
  return EditorView.inputHandler.of((view, from, to, text) => {
    if (text !== ']' || view.composing || from !== to) return false
    const line = view.state.doc.lineAt(from)
    const before = view.state.doc.sliceString(line.from, from)
    const m = CLOSED.exec(before)
    if (!m) return false
    const title = m[1]!.trim()
    if (!title) return false
    const dir = opts.notePath().split('/').slice(0, -1).join('/')
    const target = opts.resolve?.(title) ?? { title, path: `${dir ? `${dir}/` : ''}${title}.md` }
    const start = from - m[0].length
    const insert = linkFor(opts, target)
    view.dispatch({
      changes: { from: start, to, insert },
      selection: { anchor: start + insert.length },
      userEvent: 'input.complete',
    })
    return true
  })
}

/** "linked from 2 notes", the hint on a target nobody has made yet. */
export function linkedFrom(count: number): string {
  return `linked from ${count} ${count === 1 ? 'note' : 'notes'}`
}

/** The notes `[[` and `@` offer, as rows. */
export async function linkRows(opts: LinkOptions, query: string): Promise<PickRow[]> {
  return opts.search ? rowsOf(opts, await opts.search(query)) : []
}

/** The days the Day link action offers: the app's own list of days, or the day notes the search finds. */
export async function dayRows(opts: LinkOptions, query: string): Promise<PickRow[]> {
  const found = opts.days ? await opts.days(query) : opts.search ? (await opts.search(query)).filter((t) => t.daily) : []
  return rowsOf(opts, found)
}

function rowsOf(opts: LinkOptions, found: LinkTarget[]): PickRow[] {
  return found.map((t, i) => ({
      key: `${t.kind ?? 'note'}:${t.id ?? t.path}:${i}`,
      label: t.kind === 'create' ? `Create \u201C${t.title}\u201D` : t.title,
      kind: t.daily && (t.kind ?? 'note') === 'note' ? 'day' : (t.kind ?? 'note'),
      detail: t.kind === 'missing' ? linkedFrom(t.count ?? 0) : undefined,
      group: t.kind === 'mention' ? 'Mentioned in' : undefined,
      snippet: t.kind === 'mention' ? t.snippet : undefined,
      apply: (view: EditorView, from: number, to: number) => {
        opts.chosen?.(t)
        const insert = linkFor(opts, t)
        view.dispatch({ changes: { from, to, insert }, selection: { anchor: from + insert.length }, userEvent: 'input.complete' })
      },
    }))
}
