/**
 * The open note's style as state: the frontmatter pairs the editor reports (never the body), the page style read from them, and the one way to change it, `set`, which writes the keys into the
 * note's text as a single undoable change through the editor (so it follows the same operation-based save and the live room as any edit). Nothing is kept anywhere else: the note's file is the
 * only record of its style, and this object forgets it when the editor goes. The Style sheet (StyleSheet.svelte) and the bottom bar's Style entry read and write through this.
 */
import { type ChangeSpec, type TransactionSpec } from "@codemirror/state";
import { type EditorView } from "@codemirror/view";
import { caretFacts, convertChanges, toggleBold, type BlockType } from "./blocks";
import { caretWatch } from "./watch";
import { patchFrontmatter, type StylePatch } from "./keys";
import { pageStyle, type PageStyle } from "./page";

/** What the host editor gives the store to write with. All of it is the editor's existing handle (`BlockEditor`); nothing here needs a change in the editor package. */
export interface StyleHost {
  /** The editor's whole text (read once per style change, never per key). */
  text(): string;
  /** Applies one change from code (origin `command`, undoable). */
  apply(c: ChangeSpec): void;
  /** The editor's view: state for the block commands, dispatch for bold, and the one listener the block tab adds. */
  view: EditorView;
  /** Dispatches a transaction with the `command` origin. */
  run(spec: TransactionSpec): void;
}

class NoteStyle {
  /** The frontmatter pairs of the open note, as the editor last reported them. */
  fm = $state<Record<string, string>>({});
  /** True on a phone: the page is always full width there. */
  phone = $state(false);
  /** True when the note cannot be edited (a view-only share): the Style sheet shows the style and changes nothing. */
  readOnly = $state(false);
  /** Bumped when an editor attaches (a note opened), so a watch made for the old editor is made again. */
  epoch = $state(0);
  #host: StyleHost | null = null;

  /** The page style the card draws. */
  get page(): PageStyle { return pageStyle(this.fm, this.phone); }

  /** What the block tab shows: the kind of the block under the caret and whether the selection is bold; filled while a watch is open. */
  caret = $state<{ type: BlockType; bold: boolean }>({ type: "p", bold: false });

  /** Starts watching the caret (the block tab calls it while it is on screen); returns the way to stop. Nothing is added to the editor until the first call, and nothing runs per key while nobody watches. */
  watchCaret(): () => void {
    const host = this.#host;
    if (!host) return () => {};
    const apply = (f: { type: BlockType; bold: boolean }) => { if (f.type !== this.caret.type || f.bold !== this.caret.bold) this.caret = f; };
    return caretWatch(host.view, apply);
  }
  /** Turns the block under the caret (or the selected lines) into another kind, as one undoable change. */
  convert(type: BlockType): void { const h = this.#host; if (h && !this.readOnly) h.apply(convertChanges(h.view.state, type)); }
  /** Toggles bold on the selection, as one undoable change. */
  toggleBold(): void { const h = this.#host; if (h && !this.readOnly) h.run(toggleBold(h.view.state)); }

  /** Called by the note editor when it is built; returns the way to let go. */
  attach(host: StyleHost): () => void {
    this.#host = host;
    this.epoch++;
    return () => { if (this.#host === host) { this.#host = null; this.fm = {}; } };
  }

  /** The editor reports the frontmatter (once at build, then when a change touches the block). */
  report(fm: Record<string, string>): void {
    const keys = Object.keys(fm), have = Object.keys(this.fm);
    if (keys.length === have.length && keys.every((k) => this.fm[k] === fm[k])) return;
    this.fm = fm;
  }

  /** Writes keys (a value) or removes them (null) in one change; false when there is nothing to write or the note is read only. */
  set(patch: StylePatch): boolean {
    const host = this.#host;
    if (!host || this.readOnly) return false;
    const change = patchFrontmatter(host.text(), patch);
    if (!change) return false;
    host.apply(change);
    // Show the change at once; the editor's own report (when a change touches the block) then confirms it, or corrects it if the block could not be read.
    const next = { ...this.fm };
    for (const [k, v] of Object.entries(patch)) { if (v == null) delete next[k]; else next[k] = v; }
    this.fm = next;
    return true;
  }
}

/** The open note's style. */
export const noteStyle = new NoteStyle();
