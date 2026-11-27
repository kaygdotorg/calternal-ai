/**
 * The app switcher's order (the apps and the person's own sidebars), and the one way it changes: dragging in the strip, Alt+Left/Right on an item, or the item's menu. The order is saved with the mockup's other arrangement (`tweaks.setOrder`), so a reload keeps it.
 */
import { APPS } from "./data/apps";
import { arr } from "./arrange.svelte";
import { tweaks } from "./tweaks.svelte";

export const strip = {
  /** Every item's id in the person's order; anything new goes to the end. */
  ids(): string[] {
    const all = [...APPS.map((a) => a.id), ...arr.custom.map((c) => c.id)], own = tweaks.own;
    return own ? [...own.filter((i) => all.includes(i)), ...all.filter((i) => !own.includes(i))] : all;
  },
  /** Puts an item at an index. */
  place(id: string, index: number) {
    const ids = this.ids(), from = ids.indexOf(id); if (from < 0) return;
    ids.splice(from, 1); ids.splice(Math.max(0, Math.min(ids.length, index)), 0, id); tweaks.setOrder(ids);
  },
  /** Moves an item one place left (-1) or right (+1). */
  move(id: string, by: number) { this.place(id, this.ids().indexOf(id) + by); },
};
