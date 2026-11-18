/**
 * The density of a transparent window (the old Calternal's `frostRange`, `origin/review/switcher-a-pills` `apps/mockup/src/lib/behind.ts`; its rule was "the page is always frosted glass, as frosted as still reads and no less"): the page's paper laid over what is behind
 * it at the least density `d` at which the page's body ink reaches 4.5:1 and its secondary ink 3:1 against the worst colour behind it (an 18-step bisection, `FLOOR`), plus a taste margin of 0.06 (`MARGIN`); no white sheet in light, so the contrast comes from the page's own
 * density and never from a tint over the picture. What is behind a canvas window is the one background the sidebars sit on, which is already blurred and veiled, so the window needs no blur of its own and no live backdrop filter: it is only this tint. The old app took the
 * extremes from its backdrop's own colours; here they are the 1st and 99th percentile pixels of the background under its veil (appearance/wallpaper.ts `wallpaperExtremes`). Cost: 18 bisection steps, once per wallpaper or scheme change.
 */
import { contrast, type Rgb } from "../appearance/accent";

/** The contrast floors the page's words must reach (WCAG), and the taste margin above the least readable density. */
export const FLOOR = { body: 4.5, secondary: 3 }, MARGIN = 0.06;

const over = (paper: Rgb, back: Rgb, d: number): Rgb => [0, 1, 2].map((i) => paper[i]! * d + back[i]! * (1 - d)) as Rgb;
const worst = (d: number, paper: Rgb, ink: Rgb, lo: Rgb, hi: Rgb) => Math.min(contrast(ink, over(paper, lo, d)), contrast(ink, over(paper, hi, d)));

/**
 * The density of the paper over the glass for a frosted window: the least at which `ink` reaches 4.5:1 and `ink3` 3:1 against the darkest and lightest wallpaper colour, plus the margin; 1 when not even solid paper reaches it. `least` is the lowest tried
 * (0: no tint at all while the words already read, as the old window card passed).
 */
export function frostDensity(paper: Rgb, ink: Rgb, ink3: Rgb, lo: Rgb, hi: Rgb, least = 0): number {
  const ok = (d: number) => worst(d, paper, ink, lo, hi) >= FLOOR.body && worst(d, paper, ink3, lo, hi) >= FLOOR.secondary;
  if (!ok(1)) return 1;
  if (ok(least)) return Math.min(1, least + MARGIN);
  let [a, b] = [least, 1];
  for (let i = 0; i < 18; i++) { const m = (a + b) / 2; if (ok(m)) b = m; else a = m; }
  return Math.min(1, Math.round((Math.ceil(b * 100) / 100 + MARGIN) * 100) / 100);
}
