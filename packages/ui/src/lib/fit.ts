/**
 * Which pills of a row give up their words so the row fits (the decision behind `PillRow`). Pure: widths in, a set out, so the rule can be read and tested without a layout.
 *
 * The rule (docs/ui-design-system.md, "Adaptive rows"): as the room shrinks, pills drop from icon and text to icon only, one at a time, in the stated order (`drop`: the first named gives up its words first);
 * a pill in `keep` holds something the person chose (a non-default value) and gives up its words only after every other pill has, and only if the row still does not fit.
 */

/** The widths of one pill in its two forms, in px. */
export interface PillWidths {
  /** Icon and text. */
  full: number;
  /** Icon only. */
  compact: number;
}

/**
 * The ids to show icon-only. `ids` is the row's order, `widths` each pill's two widths, `avail` the room, `gap` the space between pills, `drop` the order words are given up in (ids not named drop last, in row
 * order), `keep` the ids that give up their words last, `floor` the width under which every pill not in `keep` is icon only (0: none). Returns the empty set when everything fits in full.
 */
export function collapse<T extends string>(ids: readonly T[], widths: Readonly<Record<T, PillWidths>>, avail: number, gap: number, drop: readonly T[], keep: readonly T[] = [], floor = 0): Set<T> {
  const need = (c: Set<T>) => ids.reduce((s, id) => s + (c.has(id) ? widths[id].compact : widths[id].full), 0) + Math.max(0, ids.length - 1) * gap;
  const order: T[] = [...drop.filter((id) => ids.includes(id)), ...ids.filter((id) => !drop.includes(id))];
  const rank = [...order.filter((id) => !keep.includes(id)), ...order.filter((id) => keep.includes(id))];
  const out = new Set<T>();
  // Below `floor` (a phone) every pill that holds nothing chosen is icon only whether or not the words would fit.
  if (avail < floor) for (const id of rank) if (!keep.includes(id) && widths[id].compact < widths[id].full) out.add(id);
  for (const id of rank) {
    if (need(out) <= avail) break;
    if (widths[id].compact < widths[id].full) out.add(id);
  }
  return out;
}
