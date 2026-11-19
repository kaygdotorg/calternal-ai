/**
 * The Style sheet's block tab needs to know the kind of the block under the caret while it is on screen. Nothing may run per keystroke otherwise, so the editor gets ONE listener, added the first
 * time a tab watches and never removed (CodeMirror cannot take an appended extension out), and that listener returns at its first line while nobody is watching: a keystroke then pays one
 * boolean test. Watching is counted, so two tabs or a quick remount cannot switch it off under each other.
 */
import { StateEffect, StateField } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { caretFacts, type BlockType } from "./blocks";

type Facts = { type: BlockType; bold: boolean };
/** The watchers of each editor. They outlive a replaced text (opening another note rebuilds the editor's state and drops the appended listener), so a watch checks the marker and installs again. */
const watchers = new WeakMap<EditorView, Set<(f: Facts) => void>>();
/** Present in a state exactly when the listener below is installed in it. */
const installed = StateField.define<true>({ create: () => true, update: (v) => v });

/** Calls `cb` with the caret's facts now and on every selection or text change until the returned function is called. Call it again after the note changes (the sheet does). */
export function caretWatch(view: EditorView, cb: (f: Facts) => void): () => void {
  let set = watchers.get(view);
  if (!set) { set = new Set(); watchers.set(view, set); }
  const all = set;
  if (!view.state.field(installed, false)) {
    view.dispatch({ effects: StateEffect.appendConfig.of([installed, EditorView.updateListener.of((u) => {
      if (!all.size || !(u.selectionSet || u.docChanged)) return;
      const f = caretFacts(u.state);
      for (const w of all) w(f);
    })]) });
  }
  all.add(cb);
  cb(caretFacts(view.state));
  return () => all.delete(cb);
}
