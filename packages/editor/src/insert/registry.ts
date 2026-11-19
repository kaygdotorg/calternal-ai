/**
 * The one list of block actions (#471). Every way of making a block reads it:
 * the `/` menu in the text and the Insert sheet in the bottom bar. A block
 * type is a plugin: it calls {@link registerBlock} once and shows up in both,
 * with no other change.
 *
 * To add a block (a table, say):
 *
 *   registerBlock({
 *     id: 'table', label: 'Table', group: 'Basic', icon: 'table',
 *     keywords: ['grid', 'rows'],
 *     markdown: '| Column | Column |\n| --- | --- |\n|  |  |',
 *     caret: 2, // inside the first cell
 *   })
 *
 * What is written is plain markdown, put where the caret's line is: into an
 * empty line, or on a new line after the line the caret is on. An action with
 * `pick` writes no block: it types the picker's trigger (`[[`, `@`, `#`) at
 * the caret and the same inline picker that typing it opens takes over.
 */

import type { EditorView } from '@codemirror/view'

export type Group = 'Basic' | 'Lists' | 'Media' | 'Links' | 'Embeds'
/** The order the groups show in. A plugin can use any of these. */
export const GROUPS: Group[] = ['Basic', 'Lists', 'Media', 'Links', 'Embeds']

/** What a picker chooses: a note, a day, a tag. */
export type PickKind = 'note' | 'day' | 'tag'

export interface BlockAction {
  id: string
  label: string
  group: Group
  /** A name from the shared icon set. */
  icon: string
  /** A small mark on the icon, for the heading levels. */
  badge?: string
  /** Other words people look it up by. */
  keywords: string[]
  /** The markdown it writes, as the file holds it. A function when it depends on now (a time). */
  markdown: string | (() => string)
  /** Where the caret lands inside the markdown (a UTF-16 offset); the end when left out. */
  caret?: number
  /** The typed shortcut that does the same thing, shown as a hint. Only real ones. */
  shortcut?: string
  /** Starts a picker instead of writing a block: types its trigger at the caret. */
  pick?: { kind: PickKind; trigger: string }
  /** Does something else than write markdown (a table the grid makes, a file chooser the app opens): called with the editor, and nothing is written for it. */
  run?: (view: EditorView) => void
}

let actions: BlockAction[] = []
const listeners = new Set<() => void>()
const changed = () => listeners.forEach((f) => f())

/** A block type registers itself here. Registering the same id again replaces it. Returns the undo. */
export function registerBlock(a: BlockAction): () => void {
  const i = actions.findIndex((x) => x.id === a.id)
  actions = i >= 0 ? actions.map((x, j) => (j === i ? a : x)) : [...actions, a]
  changed()
  return () => {
    actions = actions.filter((x) => x !== a)
    changed()
  }
}

/** Every registered action, in the order they registered. The same array until one registers. */
export const blockActions = (): readonly BlockAction[] => actions

export const blockById = (id: string): BlockAction | undefined => actions.find((a) => a.id === id)

/** Called when an action registers or goes (the Insert sheet redraws). Returns the undo. */
export function onBlockActions(f: () => void): () => void {
  listeners.add(f)
  return () => void listeners.delete(f)
}
