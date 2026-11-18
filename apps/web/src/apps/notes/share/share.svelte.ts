/**
 * Which item's Share sheet is open, and the button it opened from. One place, so the Share pill in a note's header and the one in a canvas window open the same sheet:
 * a pill calls `openShare(item, title, button)` and the `ShareHost` mounted by the item's own app draws it. Closing returns focus to the button (the Sheet and Popover do that).
 */
import type { ItemRef } from "./sharing";

/** The open sheet: null when none is open. */
export const shareUi = $state<{ item: ItemRef | null; title: string; trigger: HTMLElement | null }>({ item: null, title: "", trigger: null });

/** Opens the Share sheet for `item`, titled `title`, from `trigger` (the sheet grows out of it on a desktop). */
export function openShare(item: ItemRef, title: string, trigger: HTMLElement | null): void {
  shareUi.item = item;
  shareUi.title = title;
  shareUi.trigger = trigger;
}

/** Closes the Share sheet. */
export function closeShare(): void {
  shareUi.item = null;
  shareUi.trigger = null;
}
