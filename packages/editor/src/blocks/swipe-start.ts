/**
 * When a sideways swipe starts (#397, #400). One rule, shared by block swipe-select in notes and
 * by swipe rows in the lists, so a finger behaves the same everywhere:
 *
 * - never from the screen's left 24 px (that is the phone's swipe back);
 * - only after 10 px sideways, more than twice the vertical, within 300 ms. A longer, still hold
 *   is a hold instead, and a vertical move is a scroll.
 */

/** A swipe starting this close to the screen's left edge is the phone's swipe back, not ours. */
export const BACK_EDGE = 24
/** How far sideways claims the touch as a swipe, if it got there quickly. */
export const CLAIM = 10
/** A swipe starts moving within this long; slower is a hold. */
export const QUICK = 300

/** Can a touch that began at `x` still become a swipe? */
export const canStartAt = (x: number): boolean => x > BACK_EDGE

/** Has a move of (`dx`, `dy`) px, `ms` after the touch began, become a swipe? Signs don't matter. */
export const claimsSwipe = (dx: number, dy: number, ms: number): boolean =>
  ms < QUICK && Math.abs(dx) >= CLAIM && Math.abs(dx) > 2 * Math.abs(dy)

/** Is it clearly something else (a scroll, or a hold that moved)? */
export const isNotSwipe = (dx: number, dy: number, ms: number): boolean => Math.abs(dy) >= CLAIM || ms >= QUICK
