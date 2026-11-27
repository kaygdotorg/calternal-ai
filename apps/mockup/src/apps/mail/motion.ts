/**
 * Motion shared by the Mail, Calendar and Contacts mockups, all from the design system's tokens (read when an animation starts, so reduced motion's 1 ms is honoured) and all interruptible (a Svelte transition reverses from where it is when the state flips back).
 *  - `unfold`: a block opening (a folded quote, a header's details, the Cc row). ORDER: the height opens over `--dur-base` on `--ease-out`; the content fades in over the last 70 % of that, so text never shows in a box that is still too small to hold it. Closing is the same backwards.
 *  - `arrive`: a window's content appearing (the compose form, a new page). ORDER: after `delay` (a fraction of `--dur-morph`, so it starts once the shell's window has mostly grown), it fades in while rising 6 px over `--dur-base` on `--ease-out`.
 */
import { ms as tokenMs } from "../../motion";
import { tween } from "../../motion";

/** Height 0 to natural, then opacity, as a Svelte transition function. */
export function unfold(node: Element) {
  const o = tween(), h = (node as HTMLElement).scrollHeight, cs = getComputedStyle(node), mt = parseFloat(cs.marginTop), mb = parseFloat(cs.marginBottom);
  return { duration: o.duration, easing: o.easing, css: (t: number) => `overflow:hidden;height:${t * h}px;margin-top:${t * mt}px;margin-bottom:${t * mb}px;opacity:${Math.max(0, (t - 0.3) / 0.7)}` };
}
/** Fade and rise, starting `after` of the way through `--dur-morph` (0 to 1). */
export function arrive(_: Element, { after = 0 }: { after?: number } = {}) {
  const o = tween();
  return { delay: Math.round(tokenMs("--dur-morph", 350) * after), duration: o.duration, easing: o.easing, css: (t: number) => `opacity:${t};translate:0 ${(1 - t) * 6}px` };
}
