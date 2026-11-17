/**
 * The pill bounce, defined once: after a click or tap the control rebounds from its pressed size (--press-scale) past rest by a few percent and
 * settles, over --dur-slow. Pointer and touch only: a keyboard activation (`detail` 0, or the keyboard input modality) never moves, and neither
 * does reduced motion. Interruptible: a new press cancels the running rebound. Cost: one Web Animation per click, no listeners while idle beyond `click`.
 */
import { modality } from "./input";

const OVERSHOOT = 1.035;

/** Svelte action: `use:bounce` on a pill-shaped control. */
export function bounce(node: HTMLElement) {
  let anim: Animation | undefined;
  const press = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--press-scale")) || 0.96;
  const onclick = (e: MouseEvent) => {
    if (e.detail === 0 || modality() !== "pointer" || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    anim?.cancel();
    const ms = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--dur-slow")) || 300;
    anim = node.animate?.([{ scale: press }, { scale: OVERSHOOT, offset: 0.45 }, { scale: 1 }], { duration: ms, easing: "cubic-bezier(0.23, 1, 0.32, 1)" });
  };
  const onpointerdown = () => anim?.cancel();
  node.addEventListener("click", onclick);
  node.addEventListener("pointerdown", onpointerdown);
  return { destroy() { node.removeEventListener("click", onclick); node.removeEventListener("pointerdown", onpointerdown); anim?.cancel(); } };
}
