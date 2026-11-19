/**
 * Vim mode (#538): a plugin, off unless turned on in Settings, loaded only then.
 *
 * Vim itself is `@replit/codemirror-vim` (MIT): its motions, operators, counts, registers, `.`,
 * marks, text objects, visual modes and `:` commands. This folder fits it to Calternal:
 *
 * - commands.ts: undo is this person's own (ADR 0022), `V` is a block selection, `:w`, `:q`, `it`;
 * - keys.ts: Esc in normal mode, the pickers, and keys on a block selection;
 * - command-line.ts: `:`, `/` and `?` are typed in the app's bar;
 * - glue.ts: the mode on the editor, undo steps, hidden properties, other devices' changes;
 * - caret.ts: the caret's shape for each mode.
 *
 * Column selection (Ctrl-v) is several selections at once, which Calternal otherwise keeps to one.
 */

import { EditorState, type Extension, Prec } from '@codemirror/state'
import { vim } from '@replit/codemirror-vim'
import { vimCaret } from './caret'
import { setUpVim } from './commands'
import { vimEdits, vimGlue } from './glue'
import type { VimHost } from './host'
import { vimKeys } from './keys'

export type { CommandLine, VimHost } from './host'

export function vimMode(host: VimHost): Extension {
  setUpVim()
  return [
    vimKeys(host),
    // Before the editor's and the app's keymaps: in Vim mode a key is Vim's first.
    Prec.high(vim()),
    vimGlue(host),
    vimEdits,
    vimCaret,
    EditorState.allowMultipleSelections.of(true),
  ]
}
