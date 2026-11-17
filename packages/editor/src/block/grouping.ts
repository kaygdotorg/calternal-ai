/**
 * The block editor's undo grouping, made the same as a text field's (`text/grouping.ts`): a paste or a drop is a step of its own, and so is the typing that follows it (CodeMirror alone lets
 * that typing join the paste's step). Typing, deleting and cutting stay joined while they keep coming; CodeMirror's own history already isolates the paste itself.
 */
import { isolateHistory } from "@codemirror/commands";
import { EditorState, StateField } from "@codemirror/state";

/** True when the last change was a paste or a drop. */
const afterPaste = StateField.define<boolean>({
  create: () => false,
  update: (v, tr) => (tr.docChanged ? tr.isUserEvent("input.paste") || tr.isUserEvent("input.drop") : v),
});

/** Typing right after a paste or a drop starts a new undo step. */
export const grouping = [afterPaste, EditorState.transactionExtender.of((tr) => (tr.docChanged && tr.startState.field(afterPaste, false) && tr.isUserEvent("input.type") ? { annotations: isolateHistory.of("before") } : null))];
