/**
 * The accent colour, inferred from the wallpaper (its dominant hue and chroma, read once by appearance/wallpaper.ts) and guarded for contrast in both
 * schemes, so it is never a fixed brown and never unreadable. Lightness is searched, not fixed: from light to dark until all three uses hold:
 *   - white text on the accent as a fill (buttons, the send button): 4.5:1
 *   - the accent as text in the light scheme (`--accent` = the base mixed 20 % toward black) on the light paper: 4.5:1
 *   - the accent as text in the dark scheme (the base mixed 28 % toward white) on the dark paper: 4.5:1
 * Chroma is lifted to read as colour, kept between 0.09 and 0.17 so it never turns neon, and reduced where the colour would leave the sRGB gamut.
 * Cost: at most 30 iterations of a few multiplications, once per wallpaper change.
 */

/** sRGB, 0..255 each. */
export type Rgb = [number, number, number];

/** OKLCH to sRGB (0..255), or null when outside the sRGB gamut. */
export function oklchToRgb(L: number, C: number, hDeg: number): Rgb | null {
  const h = (hDeg * Math.PI) / 180;
  const a = C * Math.cos(h), b = C * Math.sin(h);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const lin = [4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s];
  if (lin.some((v) => v < -0.0005 || v > 1.0005)) return null;
  return lin.map((v) => Math.round(255 * (v <= 0.0031308 ? 12.92 * Math.max(0, v) : 1.055 * Math.min(1, v) ** (1 / 2.4) - 0.055))) as Rgb;
}

const rel = (c: Rgb): number => {
  const f = (v: number) => ((v /= 255) <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2]);
};

/** WCAG contrast ratio of two colours. */
export const contrast = (a: Rgb, b: Rgb): number => {
  const [x, y] = [rel(a), rel(b)].sort((p, q) => q - p);
  return (x! + 0.05) / (y! + 0.05);
};

const mix = (a: Rgb, b: Rgb, t: number): Rgb => [0, 1, 2].map((i) => Math.round(a[i]! + (b[i]! - a[i]!) * t)) as Rgb;
const WHITE: Rgb = [255, 255, 255];
const BLACK: Rgb = [0, 0, 0];
const PAPER_LIGHT: Rgb = [247, 247, 247]; // tokens light `paper`
const PAPER_DARK: Rgb = [31, 31, 31]; // tokens dark `paper`

/** What the accent is drawn as in each use, for a base colour: fill with white text, text in light, text in dark. */
export function uses(base: Rgb): { fill: number; light: number; dark: number } {
  return { fill: contrast(WHITE, base), light: contrast(mix(base, BLACK, 0.2), PAPER_LIGHT), dark: contrast(mix(base, WHITE, 0.28), PAPER_DARK) };
}

/**
 * The accent base colour (`#rrggbb`) for a hue (degrees) and chroma: the lightest that passes every use above. A grey wallpaper (no hue) gets a calm
 * blue. Returns the colour and the contrast it achieves in each use.
 */
export function accentFor(hue: number | null, chroma: number | null): { hex: string; contrast: { fill: number; light: number; dark: number } } {
  const h = hue ?? 250;
  const c0 = Math.min(0.17, Math.max(0.09, (chroma ?? 0.1) * 1.4));
  let best: Rgb | null = null;
  for (let L = 0.64; L >= 0.34; L -= 0.01) {
    let rgb: Rgb | null = null;
    for (let c = c0; c >= 0.03 && !rgb; c -= 0.01) rgb = oklchToRgb(L, c, h);
    if (!rgb) continue;
    best = rgb;
    const u = uses(rgb);
    if (u.fill >= 4.5 && u.light >= 4.5 && u.dark >= 4.5) break;
  }
  const rgb = best ?? [59, 110, 168];
  return { hex: `#${rgb.map((v) => v.toString(16).padStart(2, "0")).join("")}`, contrast: uses(rgb) };
}
