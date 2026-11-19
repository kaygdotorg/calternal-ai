/**
 * What Vim mode (#538) asks of the app. The editor knows Vim; the app knows what a block
 * selection means (its bar, its actions) and where a command line is typed (the bottom bar).
 */

import type { EditorView } from '@codemirror/view'
import type { Span } from '../blocks/selected'

/**
 * A command line Vim wants typed: `:` for a command, `/` or `?` to search. The app shows it as a
 * field (the bottom bar morphs into it) and passes the field's keys back here.
 */
export interface CommandLine {
  /** What the field starts with: `:`, `/` or `?`. */
  readonly prefix: string
  /** Its text to begin with (`'<,'>` after a visual selection). */
  readonly value: string
  /** A key in the field. Enter runs the line and Esc drops it; both close it. */
  keydown(e: KeyboardEvent, value: string): void
  /** After a key: a search highlights its matches as you type. */
  keyup(e: KeyboardEvent, value: string): void
  /** The field lost focus: the line is dropped. */
  blur(): void
  /** Set by the field: Vim changes the text itself (↑ and ↓ walk the history). */
  onValue: ((value: string) => void) | null
}

export interface VimHost {
  /** Selects whole blocks the app's way (its block bar, the block look). `head` is the end that moves. */
  selectBlocks(view: EditorView, spans: Span[], head: 'start' | 'end'): void
  /** Deletes the selected blocks (`d` on a block selection). */
  deleteBlocks(view: EditorView): void
  /** Nests the selected list items one level in (1) or out (-1). */
  nest(view: EditorView, dir: 1 | -1): void
  /** Shows a command line, or hides it (null). */
  commandLine(line: CommandLine | null): void
  /** A short message: `:w`'s acknowledgement, "Pattern not found". */
  say(text: string): void
  /** `:q`: done with this note. */
  quit(view: EditorView): void
}
