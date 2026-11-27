/** Motion for the mockup, from the design system's tokens (read when an animation starts, so reduced motion's 1 ms is honoured): a duration in ms, and the easing token as a function for the places that need one in script (Svelte's transitions and `animate:flip`). */
import { tokenEase, tokenMs } from "@calternal/ui";

/** A cubic-bezier CSS easing (`cubic-bezier(0.23, 1, 0.32, 1)`) as a function of progress; anything else is linear. */
export function bezier(css: string): (t: number) => number {
  const m = /cubic-bezier\(\s*([-\d.]+)\s*,\s*([-\d.]+)\s*,\s*([-\d.]+)\s*,\s*([-\d.]+)\s*\)/.exec(css);
  if (!m) return (t) => t;
  const [x1, y1, x2, y2] = [1, 2, 3, 4].map((i) => parseFloat(m[i]!)) as [number, number, number, number];
  const ax = 1 - 3 * x2 + 3 * x1, bx = 3 * x2 - 6 * x1, cx = 3 * x1, ay = 1 - 3 * y2 + 3 * y1, by = 3 * y2 - 6 * y1, cy = 3 * y1;
  const sx = (u: number) => ((ax * u + bx) * u + cx) * u, sy = (u: number) => ((ay * u + by) * u + cy) * u;
  return (t) => { if (t <= 0 || t >= 1) return t <= 0 ? 0 : 1; let u = t; for (let i = 0; i < 8; i++) { const d = (3 * ax * u + 2 * bx) * u + cx; if (Math.abs(d) < 1e-6) break; u -= (sx(u) - t) / d; } return sy(Math.min(1, Math.max(0, u))); };
}
/** The token duration and `--ease-out`, as Svelte transition parameters. */
/** Whether the person asks for reduced motion (the system setting). Everything script-driven asks here, because a CSS rule cannot shorten an animation a script runs. */
export const reduced = (): boolean => typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
/** A token duration in ms, 1 under reduced motion. */
export const ms = (name: string, fallback: number): number => (reduced() ? 1 : tokenMs(name, fallback));
export const tween = (dur = "--dur-base", fallback = 220) => ({ duration: ms(dur, fallback), easing: bezier(tokenEase("--ease-out", "cubic-bezier(0.23, 1, 0.32, 1)")) });
/** A row arriving after a tab switch: it fades in while rising a few pixels, never a different row's exit (the old rows go at once). */
export const rise = (_: Element) => { const t = tween(); return { duration: t.duration, easing: t.easing, css: (p: number) => `opacity:${p};translate:0 ${(1 - p) * 6}px` }; };
