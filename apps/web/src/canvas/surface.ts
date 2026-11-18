/**
 * The colour of a canvas window (docs/ui-shell-status.md, "The canvas experiment"): Craft's "document colour", derived from the wallpaper instead of chosen. The hue is the wallpaper's accent hue (appearance/accent.ts
 * already read it from the picture); the lightness and chroma are the values measured on Craft's five dark and light documents: a deep, nearly neutral tone in dark (OKLCH L 0.22, C 0.02), a pale one in light (L 0.935, C 0.022). Ink
 * is then solved against that exact colour: the scheme's primary ink when it reaches 7:1, else the other one, and the two quiet inks moved toward it until they reach 5.5:1 and 4.5:1, the floor of appearance/veil.ts.
 * Cost: a few dozen multiplications, once per wallpaper or scheme change.
 */
import { contrast, oklchToRgb, type Rgb } from "../appearance/accent";

/** The measured lightness and chroma of the window surface per scheme (see the status document for where they come from). */
export const DOC = { dark: { L: 0.22, C: 0.02 }, light: { L: 0.935, C: 0.022 } } as const;
const INK = { dark: [235, 235, 235], light: [38, 38, 38] } as const satisfies Record<string, Rgb>;

/** Hue (degrees) of an `#rrggbb` colour in OKLCH; null for a grey. */
export function hueOf(hex: string): number | null {
  const v = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  const [r, g, b] = v as [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b), m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b), s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  if (Math.hypot(a, bb) < 0.004) return null;
  return ((Math.atan2(bb, a) * 180) / Math.PI + 360) % 360;
}

/** A window's colours. */
export interface WindowColours { bg: Rgb; ink: Rgb; ink2: Rgb; ink3: Rgb }

const mix = (a: Rgb, b: Rgb, t: number): Rgb => [0, 1, 2].map((i) => Math.round(a[i]! + (b[i]! - a[i]!) * t)) as Rgb;

/** The smallest step from `tok` toward `ink` that reaches `target` against `bg` (the token itself when it already does). */
function toward(tok: Rgb, ink: Rgb, bg: Rgb, target: number): Rgb {
  for (let t = 0; t <= 1.0001; t += 0.05) { const c = mix(tok, ink, t); if (contrast(c, bg) >= target) return c; }
  return ink;
}

/** The surface and inks for a scheme and accent hue (null: a calm blue-grey). */
export function windowColours(scheme: "light" | "dark", hue: number | null): WindowColours {
  const { L, C } = DOC[scheme];
  let bg: Rgb | null = null;
  for (let c = C; c >= 0 && !bg; c -= 0.004) bg = oklchToRgb(L, Math.max(0, c), hue ?? 250);
  const surface = bg ?? ([scheme === "dark" ? 31 : 247, scheme === "dark" ? 31 : 247, scheme === "dark" ? 31 : 247] as Rgb);
  const own = INK[scheme], other = INK[scheme === "dark" ? "light" : "dark"];
  const ink = contrast(own, surface) >= 7 ? own : contrast(other, surface) > contrast(own, surface) ? other : own;
  const soft2: Rgb = scheme === "dark" ? [196, 196, 196] : [74, 74, 74], soft3: Rgb = scheme === "dark" ? [166, 166, 166] : [95, 95, 95];
  return { bg: surface, ink, ink2: toward(soft2, ink, surface, 5.5), ink3: toward(soft3, ink, surface, 4.5) };
}

/** `r g b` for `rgb(var(--x))`. */
export const triplet = (c: Rgb): string => c.join(" ");
