/**
 * Collision-aware placement for floating surfaces (menus, popovers, context menus): the surface is positioned against its anchor, flipped to the side
 * with more room, shifted to stay inside the window, and given a max height equal to the room it has, so nothing is ever drawn out of bounds.
 * Cost: two `getBoundingClientRect` reads, both before any write (one layout at most), then style writes that never invalidate layout: the surface sits at
 * the window's origin (`left: 0; top: 0` in its own CSS) and is moved with `translate`, so nothing reads layout again after the writes. Called when a surface opens and when the window resizes.
 */

/** A point or rectangle in viewport coordinates, or an element to measure. */
export type Anchor = Element | { left: number; top: number; right: number; bottom: number };

/** Where to put the surface relative to its anchor. `prefer` is the side tried first; `margin` is the least distance kept from the window's edges. */
export interface PlaceOptions {
  prefer?: "top" | "bottom";
  gap?: number;
  margin?: number;
  /** The surface covers its anchor instead of sitting beside it: its near edge lines up with the anchor's (bottom edge when it grows upward, top edge when downward) and it is at least as wide, so a popover that morphs out of a button starts inside the button's own box. */
  cover?: boolean;
}

/** Positions `el` (already `position: fixed; left: 0; top: 0`) against `anchor`, with `translate`. Returns the side it ended up on. */
export function place(el: HTMLElement, anchor: Anchor, { prefer = "bottom", gap = 6, margin = 8, cover = false }: PlaceOptions = {}): "top" | "bottom" {
  const a = anchor instanceof Element ? anchor.getBoundingClientRect() : anchor;
  // Reads first. A height limit from an earlier placement (a resize) has to go before the surface is measured; a first placement has none, so it costs no write.
  if (el.style.maxHeight) el.style.maxHeight = "";
  if (cover && el.style.minWidth !== `${a.right - a.left}px`) el.style.minWidth = `${a.right - a.left}px`;
  const r = el.getBoundingClientRect();
  // Covering, the surface grows from the anchor's own edge, so there is no gap and the room is measured from that edge.
  const g = cover ? 0 : gap;
  const above = (cover ? a.bottom : a.top) - margin - g;
  const below = innerHeight - (cover ? a.top : a.bottom) - margin - g;
  let side = prefer;
  if (side === "top" && r.height > above && below > above) side = "bottom";
  else if (side === "bottom" && r.height > below && above > below) side = "top";
  const room = Math.max(96, side === "top" ? above : below);
  const h = Math.min(r.height, room);
  el.style.maxHeight = `${room}px`;
  el.style.overflowY = "auto";
  const left = Math.min(Math.max(a.left, margin), Math.max(margin, innerWidth - margin - r.width));
  const top = cover ? (side === "top" ? a.bottom - h : a.top) : side === "top" ? a.top - gap - h : a.bottom + gap;
  el.style.translate = `${left}px ${top}px`;
  el.style.setProperty("--origin", `${side === "top" ? "bottom" : "top"} ${a.left < left + r.width / 2 ? "left" : "right"}`);
  return side;
}
