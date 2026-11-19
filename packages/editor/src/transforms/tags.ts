/**
 * The `#` picker (#449): typing `#` and a letter offers the tags that
 * exist, in the same glass list as `[[` and `@` (see picker.ts).
 *
 * - It waits for a letter after `#`, so "issue #12" or a lone `#` never
 *   opens it. Not in code, links or the properties block.
 * - Enter or Tab takes the highlighted tag. Tab on a tag with tags inside
 *   it completes that part instead (`#area/`) and shows what's inside.
 * - Space ends the tag and the list goes; Esc closes it. A new tag needs
 *   no row: you just keep typing.
 *
 * What is offered comes from the app ({@link TagOptions}); what matches and
 * the order are {@link rankTags}: a tag in a folder-like path is found by any
 * part of it (`#fit` finds `area/fitness`).
 */

import { syntaxTree } from '@codemirror/language'
import type { EditorState } from '@codemirror/state'
import type { EditorView } from '@codemirror/view'
import { frontmatterEnd } from '../syntax/frontmatter'
import type { PickRow } from './rows'
import { rankTags } from './tag-rank'

export interface TagOptions {
  /**
   * Every tag, and every tag above a nested one (`area` for `area/work`),
   * with the number of notes that use it or a tag inside it. Asked on
   * every keystroke while the list is open, so keep it at hand.
   */
  counts: () => ReadonlyMap<string, number>
}

/** Syntax a tag, a link or a block action can't be inside. */
const NOT_HERE = new Set(['InlineCode', 'FencedCode', 'CodeBlock', 'CodeText', 'Link', 'URL', 'Autolink', 'HTMLBlock', 'CommentBlock'])

/** True when `pos` is plain text: not in the properties, in code or in a link. */
export function inPlainText(state: EditorState, pos: number): boolean {
  if (pos < frontmatterEnd(state)) return false
  for (let n: ReturnType<ReturnType<typeof syntaxTree>['resolveInner']> | null = syntaxTree(state).resolveInner(pos, -1); n; n = n.parent)
    if (NOT_HERE.has(n.name)) return false
  return true
}

/** Puts `#tag` in place of what was typed, with a space after it unless one is there already. */
function put(view: EditorView, tag: string, from: number, to: number) {
  const next = view.state.doc.sliceString(to, to + 1)
  const insert = `#${tag}${next && /\s/.test(next) ? '' : ' '}`
  view.dispatch({
    changes: { from, to, insert },
    selection: { anchor: from + `#${tag}`.length + (insert.endsWith(' ') ? 1 : 0) },
    userEvent: 'input.complete',
  })
}

/**
 * The tags `#` offers for what was typed after it, as rows. The typed letters
 * are drawn stronger wherever they matched, even deep in a path. Tab on a tag
 * with tags inside it completes to `#tag/` and the list shows what's inside
 * (the picker looks again after the edit); anything else Tab takes as Enter.
 */
export function tagRows(opts: TagOptions, typed: string): PickRow[] {
  return rankTags(opts.counts(), typed).map((t) => ({
    key: t.tag,
    label: t.tag,
    kind: 'tag',
    count: t.count,
    kids: t.kids,
    match: t.match,
    apply: (view, from, to) => put(view, t.tag, from, to),
    tab: t.kids
      ? (view, from, to) => {
          const insert = `#${t.tag}/`
          view.dispatch({ changes: { from, to, insert }, selection: { anchor: from + insert.length }, userEvent: 'input.complete' })
        }
      : undefined,
  }))
}
