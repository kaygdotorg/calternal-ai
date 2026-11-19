/**
 * The open note's style as state: the frontmatter pairs the editor reports (never the body), the page style read from them, and the one way to change it, `set`, which writes the keys into the
 * note's text as a single undoable change through the editor (so it follows the same operation-based save and the live room as any edit). Nothing is kept anywhere else: the note's file is the
 * only record of its style, and this object forgets it when the editor goes. The Style sheet (StyleSheet.svelte) and the bottom bar's Style entry read and write through this.
 */
import { patchFrontmatter, type StylePatch } from "./keys";
import { pageStyle, type PageStyle } from "./page";

/** What the host editor gives the store to write with: the current text and an applier. */
export interface StyleHost {
  /** The editor's whole text (read once per style change, never per key). */
  text(): string;
  /** Applies one change from code (origin `command`, undoable). */
  apply(c: { from: number; to: number; insert: string }): void;
}

class NoteStyle {
  /** The frontmatter pairs of the open note, as the editor last reported them. */
  fm = $state<Record<string, string>>({});
  /** True on a phone: the page is always full width there. */
  phone = $state(false);
  /** True when the note cannot be edited (a view-only share): the Style sheet shows the style and changes nothing. */
  readOnly = $state(false);
  #host: StyleHost | null = null;

  /** The page style the card draws. */
  get page(): PageStyle { return pageStyle(this.fm, this.phone); }

  /** Called by the note editor when it is built; returns the way to let go. */
  attach(host: StyleHost): () => void {
    this.#host = host;
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
