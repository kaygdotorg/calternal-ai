/**
 * A row of the inline picker. Every trigger (`/`, `[[`, `@`, `#`) makes rows
 * of this one shape, so there is one list to draw and one keyboard model.
 * The row knows what choosing it does (`apply`), so the picker never needs to
 * know what kind of thing it is showing.
 */

import type { EditorView } from '@codemirror/view'

export type RowKind = 'action' | 'note' | 'day' | 'missing' | 'create' | 'mention' | 'tag'

export interface PickRow {
  key: string
  label: string
  kind: RowKind
  /** A name from the shared icon set (the app draws it). */
  icon?: string
  /** A small mark on the icon (a heading's level). */
  badge?: string
  /** A typed shortcut, drawn small at the end of the row. */
  hint?: string
  /** Quiet words at the end of the row ("linked from 2 notes"). */
  detail?: string
  /** A tag: the notes that use it, and whether tags sit inside it. */
  count?: number
  kids?: boolean
  /** The typed letters inside the label, drawn stronger. */
  match?: [number, number]
  /** Rows with a group show its name above the first of them. */
  group?: string
  /** A note found by its text: where the words are, with the words marked. */
  snippet?: { text: string; marks: [number, number][] }
  /** Puts the row into the text in place of `from`–`to` (the trigger and what was typed after it). */
  apply: (view: EditorView, from: number, to: number) => void
  /** Tab on the row, when it means something other than Enter (a tag with tags inside it opens it). */
  tab?: (view: EditorView, from: number, to: number) => void
}
