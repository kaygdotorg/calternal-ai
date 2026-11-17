/**
 * Collision-aware placement for floating surfaces (menus, popovers, context menus): the surface is positioned against its anchor, flipped to the side
 * with more room, shifted to stay inside the window, and given a max height equal to the room it has, so nothing is ever drawn out of bounds.
 * Cost: two `getBoundingClientRect` reads, both before any write (one layout at most), then style writes. Sideways it is moved with `translate` (the surface sits at the window's origin, `left: 0`
 * in its own CSS); vertically it is anchored by the edge that touches the anchor (`bottom` when it opens upward, `top` downward), so content that arrives later (a picker adding rows after its first
 * frame) grows away from the anchor with no second placement. Called when a surface opens and when the window resizes.
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
  /**
   * Place without measuring the surface at all: from the anchor's rectangle and the window alone, so a surface that has just been inserted (a picker opening on a key) costs no layout read. It keeps
   * `prefer` unless that side has under 180 px and the other has more, is at least `blind` px wide of room from the anchor's left edge (shifted left when the window is narrower; default 256), and can
   * never be wider than the window less its margins. Leave it off when the surface's own size matters (a menu that must flip because it does not fit).
   */
  blind?: number;
}

/** Positions `el` (already `position: fixed; left: 0; top: 0`) against `anchor`, with `translate`. Returns the side it ended up on. */
export function place(el: HTMLElement, anchor: Anchor, { prefer = "bottom", gap = 6, margin = 8, cover = false, blind }: PlaceOptions = {}): "top" | "bottom" {
  const a = anchor instanceof Element ? anchor.getBoundingClientRect() : anchor;
  if (blind) return placeBlind(el, a, prefer, gap, margin, blind);
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
  el.style.maxHeight = `${room}px`;
  el.style.overflowY = "auto";
  const left = Math.min(Math.max(a.left, margin), Math.max(margin, innerWidth - margin - r.width));
  el.style.translate = `${left}px 0`;
  if (side === "top") { el.style.top = "auto"; el.style.bottom = `${innerHeight - (cover ? a.bottom : a.top - gap)}px`; }
  else { el.style.bottom = "auto"; el.style.top = `${cover ? a.top : a.bottom + gap}px`; }
  el.style.setProperty("--origin", `${side === "top" ? "bottom" : "top"} ${a.left < left + r.width / 2 ? "left" : "right"}`);
  return side;
}

/** `place` without reading the surface: see `PlaceOptions.blind`. */
function placeBlind(el: HTMLElement, a: { left: number; top: number; right: number; bottom: number }, prefer: "top" | "bottom", gap: number, margin: number, minWidth: number): "top" | "bottom" {
  const above = a.top - margin - gap;
  const below = innerHeight - a.bottom - margin - gap;
  const other = prefer === "top" ? "bottom" : "top";
  const room = (s: "top" | "bottom") => (s === "top" ? above : below);
  const side = room(prefer) < 180 && room(other) > room(prefer) ? other : prefer;
  el.style.maxHeight = `${Math.max(96, room(side))}px`;
  el.style.maxWidth = `${innerWidth - 2 * margin}px`;
  el.style.overflowY = "auto";
  const left = Math.min(Math.max(a.left, margin), Math.max(margin, innerWidth - margin - minWidth));
  el.style.translate = `${left}px 0`;
  if (side === "top") { el.style.top = "auto"; el.style.bottom = `${innerHeight - (a.top - gap)}px`; }
  else { el.style.bottom = "auto"; el.style.top = `${a.bottom + gap}px`; }
  el.style.setProperty("--origin", `${side === "top" ? "bottom" : "top"} left`);
  return side;
}
