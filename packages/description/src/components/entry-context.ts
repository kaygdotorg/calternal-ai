import { getContext, setContext } from "svelte";
import type { ItemViewState } from "./entry-state.svelte";

/** What an entry needs from the item view that draws it: its state, and the gestures it forwards (a menu at a point, picking a message for a range, jumping to an anchor, opening an item link). */
export interface EntryCtx {
  state: ItemViewState;
  menu(anchor: string, at: { x: number; y: number }): void;
  pick(anchor: string): void;
  jump(anchor: string): void;
  link(target: { space: string; id: string }): void;
}
const KEY = Symbol("entry-view");
/** Provides the entry context to every snapshot node drawn inside an item view. */
export const provideEntry = (c: EntryCtx) => setContext(KEY, c);
/** The entry context, or undefined outside an item view (nodes then draw without entry features). */
export const useEntry = (): EntryCtx | undefined => getContext<EntryCtx | undefined>(KEY);
