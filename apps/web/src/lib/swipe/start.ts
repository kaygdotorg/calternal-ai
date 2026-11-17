/**
 * When a sideways swipe starts, ported from the existing app's `@calternal/editor/swipe-start` (#397, #400). One rule for every swipe row:
 * never from the screen's left 24 px (the phone's swipe back); only after 10 px sideways, more than twice the vertical, within 300 ms. A longer, still hold
 * is a hold instead, and a vertical move is a scroll.
 */

/** A swipe starting this close to a screen edge is the shell's (swipe back, side panel), not a row's. */
export const BACK_EDGE = 24;
/** How far sideways claims the touch as a swipe, if it got there quickly. */
export const CLAIM = 10;
/** A swipe starts moving within this long; slower is a hold. */
export const QUICK = 300;

/** Has a move of (`dx`, `dy`) px, `ms` after the touch began, become a swipe? Signs don't matter. */
export const claimsSwipe = (dx: number, dy: number, ms: number): boolean => ms < QUICK && Math.abs(dx) >= CLAIM && Math.abs(dx) > 2 * Math.abs(dy);
