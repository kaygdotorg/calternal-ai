/**
 * Collision-aware placement for floating surfaces (menus, popovers, context menus): the surface is positioned against its anchor, flipped to the side
 * with more room, shifted to stay inside the window, and given a max height equal to the room it has, so nothing is ever drawn out of bounds.
 * Cost: two `getBoundingClientRect` reads and four style writes per placement; called when a surface opens and when the window resizes.
 */

/** A point or rectangle in viewport coordinates, or an element to measure. */
export type Anchor = Element | { left: number; top: number; right: number; bottom: number };

/** Where to put the surface relative to its anchor. `prefer` is the side tried first; `margin` is the least distance kept from the window's edges. */
export interface PlaceOptions {
  prefer?: "top" | "bottom";
  gap?: number;
  margin?: number;
}

/** Positions `el` (already `position: fixed`) against `anchor`. Returns the side it ended up on. */
export function place(el: HTMLElement, anchor: Anchor, { prefer = "bottom", gap = 6, margin = 8 }: PlaceOptions = {}): "top" | "bottom" {
  const a = anchor instanceof Element ? anchor.getBoundingClientRect() : anchor;
  el.style.maxHeight = "";
  const r = el.getBoundingClientRect();
  const above = a.top - margin - gap;
  const below = innerHeight - a.bottom - margin - gap;
  let side = prefer;
  if (side === "top" && r.height > above && below > above) side = "bottom";
  else if (side === "bottom" && r.height > below && above > below) side = "top";
  const room = Math.max(96, side === "top" ? above : below);
  const h = Math.min(r.height, room);
  el.style.maxHeight = `${room}px`;
  el.style.overflowY = "auto";
  const left = Math.min(Math.max(a.left, margin), Math.max(margin, innerWidth - margin - r.width));
  el.style.left = `${left}px`;
  el.style.top = `${side === "top" ? a.top - gap - h : a.bottom + gap}px`;
  el.style.setProperty("--origin", `${side === "top" ? "bottom" : "top"} ${a.left < left + r.width / 2 ? "left" : "right"}`);
  return side;
}
