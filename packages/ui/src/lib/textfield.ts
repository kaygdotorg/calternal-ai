/**
 * Svelte action `use:textField` for a plain `<input>` or `<textarea>` that is not an `InlineEditor`: it gives the field the shared text behaviour (the editor package's `FieldEditor`): one undo history
 * for typing and pasting, the shared shortcuts, line-break-free paste in a one-line field, the input-method guard, and spelling and capitalisation by `kind`. The field stays native (accessibility,
 * input methods and autofill are the browser's). Cost: the listeners `FieldEditor` adds, none per keystroke beyond its one `input` handler.
 */
import { FieldEditor, type TextKind } from "../../../editor/src/index";

/** Options: what the field holds (default prose). `cancels` is true when Escape means "cancel" to the host, so it does not also collapse a selection. */
export interface TextFieldOptions { kind?: TextKind; cancels?: boolean }

/** Attaches the shared text behaviour to a field. */
export function textField(node: HTMLInputElement | HTMLTextAreaElement, o: TextFieldOptions = {}) {
  const fe = new FieldEditor(node, { kind: o.kind ?? "prose", escapeCollapses: !o.cancels });
  const sync = () => fe.sync(); // a framework binding may have set the value since the last keystroke
  node.addEventListener("focus", sync);
  return { destroy() { node.removeEventListener("focus", sync); fe.destroy(); } };
}
