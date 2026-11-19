// The bar's modes (#269): each thing the one bar can be (tabs, note tools, text bar, block bar,
// sheets, a toast, a question) is a small registered plugin: what it shows, what it's called, and
// what kind of thing it is for assistive tech. The bar itself never knows the list: an app
// registers its modes, the shell picks a key (pick.ts), and the bar draws whichever mode owns it.
// A new mode (multi-select, a block toolbar) is one more `register` call, nothing else.
import type { Component } from "svelte";

export type BarRole = "toolbar" | "tablist" | "dialog" | "status";

export type BarMode<Ctx> = {
  /** The mode's name; a key belongs to it if it is the name or starts with `name-`. */
  id: string;
  /** What it is, for assistive tech: a row of tools, a set of tabs, a sheet, or a notice. */
  role: BarRole;
  /** Its name for assistive tech, for this key. */
  label: (key: string, ctx: Ctx) => string;
  /** What it shows. Gets the key (a sheet's or toast's key says which one) and the app's context. */
  component: Component<{ k: string; ctx: Ctx }>;
};

/** One app's set of modes. */
export function barModes<Ctx>() {
  const modes = new Map<string, BarMode<Ctx>>();
  return {
    register(mode: BarMode<Ctx>) {
      modes.set(mode.id, mode);
    },
    /** The mode that owns this key (`sheet-insert` belongs to `sheet`). */
    of(key: string): BarMode<Ctx> | undefined {
      return modes.get(key) ?? modes.get(key.split("-")[0]);
    },
  };
}
