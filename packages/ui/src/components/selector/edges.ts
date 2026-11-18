// The capsule's two edges, as plain numbers, so the motion rules can be tested without a browser.
//
// When the selection moves, the edge on the side it's moving toward leads (a quick spring) and
// the other edge catches up (a softer one). So the capsule stretches toward the new option, then
// gathers itself there. Interrupt it and the roles are decided again from where it is right now.

import type { Snippet } from "svelte";
import type { SpringConfig } from "../../lib/spring-engine";

/** One choice of a Selector: its `value`, the `label` (always its accessible name), an optional icon, and whether it is disabled (with a `hint` saying why). */
export type SelectorOption<T> = {
  value: T;
  label: string;
  /** An icon before the label, or instead of it with `iconOnly` (the label stays the name). */
  icon?: Snippet;
  /** An icon from the host's icon set, by name (the simple case: no snippet to write). Drawn like `icon`. */
  iconName?: import("../../icons").IconName;
  iconOnly?: boolean;
  disabled?: boolean;
  /** Why it's disabled, or extra words for the tooltip. */
  hint?: string;
};

/** The capsule's two edges, in px from the track's left. */
export type Edges = { left: number; right: number };

/** The spring of the edge the capsule is moving toward: quick, so it leads. */
export const LEAD: SpringConfig = { damping: 0.86, response: 0.26 };
/** The spring of the edge that catches up: softer, so the capsule stretches and then gathers itself. */
export const TRAIL: SpringConfig = { damping: 0.92, response: 0.42 };

/** Which edge leads from the capsule's current place to the target, or null if it stays put. */
export function leadingEdge(now: Edges, target: Edges): "left" | "right" | null {
  const from = (now.left + now.right) / 2;
  const to = (target.left + target.right) / 2;
  if (Math.abs(to - from) < 0.5) return null;
  return to > from ? "right" : "left";
}

/** Spring settings for each edge for a move (both lead when nothing moves sideways: a resize). */
export function edgeConfigs(now: Edges, target: Edges): { left: SpringConfig; right: SpringConfig } {
  const lead = leadingEdge(now, target);
  if (!lead) return { left: LEAD, right: LEAD };
  return lead === "right" ? { left: TRAIL, right: LEAD } : { left: LEAD, right: TRAIL };
}

/** The next option in a direction, skipping disabled ones (wraps around, like a tab list). */
export function step(disabled: boolean[], from: number, dir: 1 | -1): number {
  const n = disabled.length;
  for (let k = 1; k <= n; k++) {
    const i = (((from + dir * k) % n) + n) % n;
    if (!disabled[i]) return i;
  }
  return from;
}
