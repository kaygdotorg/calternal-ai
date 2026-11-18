/**
 * The frosted window, ported from the old Calternal (`origin/review/switcher-a-pills`: `apps/mockup/src/lib/behind.ts` `frostRange`, `frost.svelte.ts`, `packages/ui/src/styles/panes.css` `.fg-glass`). Its rule, in the owner's words then: "the page is always
 * frosted glass, as frosted as still reads and no less". The glass is ONE layer under the whole card: `backdrop-filter: blur(20px) saturate(140%)` (`AUTO_BLUR_PX`) and the page's paper laid over it at a density (`--pane-up`) that is the least at which the
 * page's body ink reaches 4.5:1 and its secondary ink 3:1 against the worst colour behind it (bisection, `FLOOR`), plus a taste margin of 0.06 (`MARGIN`); with no white sheet in light: contrast comes from the page's own density, never from a tint over the
 * picture. The old app took the extremes from the backdrop's own colours; here they are the wallpaper's 1st and 99th percentile pixels (appearance/wallpaper.ts `wallpaperExtremes`). Cost: 18 bisection steps, once per wallpaper or scheme change.
 */
import { contrast, type Rgb } from "../appearance/accent";

/** The blur of an automatic frost (px) and the saturation the old glass used. */
export const BLUR_PX = 20, SATURATE = 1.4;
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
