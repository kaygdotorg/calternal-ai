/** What the row press is showing, shared by the gesture (lib/press.ts, which writes it) and the surface (components/PressSurface.svelte, which draws it). */
import type { MenuItem } from "../components/Menu.svelte";

/** What a row says about itself when it is pressed. Read when needed, never copied. */
export interface PressBag {
  /** The row's name, for the menu's accessible name. */
  label: string;
  /** The menu's items: the same ones the row's three dots open. */
  items: () => MenuItem[];
  /** The picked item's id. */
  onpick: (id: string) => void;
  /** What the preview card shows (a registered kind, see `registerPreview`), or none: the press then opens the menu alone. */
  preview?: { kind: string; item: unknown } | null;
  /** Reserved for the engine's drag hooks (a row that can be dragged); nothing uses it yet. */
  drag?: unknown;
}

/** One open press. */
export interface PressOpen {
  row: HTMLElement;
  bag: PressBag;
  /** A touch hold: the page dims, a preview card grows out of the row and the menu sits under it. Otherwise (right click, the menu key) the menu opens alone at `x`, `y`. */
  preview: boolean;
  x: number;
  y: number;
  /** The row's look-alike, made while the finger was still waiting for the menu's half second. */
  copy?: HTMLElement;
}

/** Not deeply reactive (`$state.raw`): the press holds DOM nodes and callbacks, which must keep their identity. */
class PressState { cur = $state.raw<PressOpen | null>(null); }
export const pressState = new PressState();
