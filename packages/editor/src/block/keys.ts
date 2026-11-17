/**
 * The block editor's keys: it EXTENDS the shared text layer instead of running its own. The shared shortcuts (`text/keys.ts`) run the shared commands (`text/commands.ts`) through the thin adapter
 * below; CodeMirror's default bindings stay for plain cursor movement, selection, deleting and undo, minus everything a text field does not do (several cursors, comment toggling, syntax-aware
 * movement, selection history, indent-aware Backspace and Enter), so a field and a note answer the same keys the same way. What a note adds on top (list continuation, Tab on a list item) is in `editor.ts`.
 */
import {
  addCursorAbove, addCursorBelow, copyLineDown, copyLineUp, cursorMatchingBracket, cursorSyntaxLeft, cursorSyntaxRight, defaultKeymap, deleteCharBackward, deleteCharBackwardStrict, deleteLine, historyKeymap,
  indentLess, indentMore, indentSelection, insertBlankLine, insertNewline, insertNewlineAndIndent, moveLineDown, moveLineUp, redo, redoSelection, selectLine, selectParentSyntax, selectSyntaxLeft, selectSyntaxRight,
  simplifySelection, toggleBlockComment, toggleComment, toggleTabFocusMode, undoSelection,
} from "@codemirror/commands";
import { keymap, type EditorView, type KeyBinding } from "@codemirror/view";
import { COMMANDS, KEYMAP } from "../text";
import { origin } from "./origin";

/** What CodeMirror binds that a text field has no counterpart for. */
const NOT_SHARED = new Set<unknown>([
  addCursorAbove, addCursorBelow, copyLineDown, copyLineUp, cursorMatchingBracket, cursorSyntaxLeft, cursorSyntaxRight, deleteLine, indentLess, indentMore, indentSelection, insertBlankLine,
  moveLineDown, moveLineUp, selectLine, selectParentSyntax, selectSyntaxLeft, selectSyntaxRight, simplifySelection, toggleBlockComment, toggleComment, toggleTabFocusMode,
  undoSelection, redoSelection, insertNewlineAndIndent, deleteCharBackward,
]);

const shared: KeyBinding[] = KEYMAP.map((k) => ({
  key: k.key,
  mac: k.mac,
  preventDefault: true,
  run: (view: EditorView): boolean => {
    const m = view.state.selection.main;
    const edit = COMMANDS[k.run](view.state.doc, { anchor: m.anchor, head: m.head });
    if (!edit) return k.run === "collapse" ? false : true;
    const c = edit.change;
    view.dispatch({ changes: c.insert === "" && c.from === c.to ? undefined : c, selection: { anchor: edit.selection.anchor, head: edit.selection.head }, userEvent: "input.command", annotations: [origin.of("command")], scrollIntoView: true });
    return true;
  },
}));

/** The bindings: the shared shortcuts, then CodeMirror's plain ones with Enter a plain newline and Backspace never deleting by indent. */
export const textKeymap = [
  keymap.of(shared),
  // Redo is Shift-Mod-Z on every platform, as in a field (CodeMirror binds it on a Mac only).
  keymap.of([{ key: "Shift-Mod-z", run: redo, preventDefault: true }, { key: "Enter", run: insertNewline, shift: insertNewline }, { key: "Backspace", run: deleteCharBackwardStrict, shift: deleteCharBackwardStrict, preventDefault: true }]),
  keymap.of([...defaultKeymap, ...historyKeymap].filter((b) => !NOT_SHARED.has(b.run) && !NOT_SHARED.has(b.shift))),
];
