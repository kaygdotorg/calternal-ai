/**
 * Themes as palettes. A theme supplies a handful of colours per scheme (paper, ink, accent and four state colours) and this file derives every
 * colour token from them, following `docs/ui-backgrounds-and-themes.md` section 3 so Swift and TypeScript can agree: secondary inks are paper and ink
 * mixed in OKLab at 82, 70 and 54 %, then guarded; accent text, the state colours' text uses and the soft accent are derived; geometry, glass, shadows,
 * motion and type are never themable. Cost: a few dozen colour conversions, once per theme or scheme change.
 */
import { contrast, type Rgb } from "./accent";

/** What a theme supplies for one scheme. Every colour is `#rrggbb`. */
export interface Palette {
  /** The surface colour (`#rrggbb`); the veil and every text colour are solved against it. */
  paper: string;
  /** The text colour on paper. */
  ink: string;
  /** The accent colour for selection, the primary button and focus. */
  accent: string;
  /** The colour of errors and destructive actions. */
  danger: string;
  /** The colour of success. */
  good: string;
  /** The colour of warnings. */
  warn: string;
  /** The colour of neutral notices. */
  info: string;
}

/** A theme: a name and a palette for each scheme (a twin; a theme with one scheme repeats its palette). */
export interface Theme {
  /** Stable id; stored with the choice. */
  id: string;
  /** The name shown in Appearance. */
  name: string;
  /** The palette used in the light scheme. */
  light: Palette;
  /** The palette used in the dark scheme. */
  dark: Palette;
  /** The licence of the theme's colours (shown in credits). */
  licence?: string;
  /** Where the theme comes from (a link), for credit. */
  source?: string;
}

const hex = (s: string): Rgb => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16)) as Rgb;
const toHex = (c: Rgb): string => `#${c.map((v) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0")).join("")}`;

// ---- OKLab, for mixing ----
const lin = (v: number) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const gam = (v: number) => 255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
function toLab(c: Rgb): [number, number, number] {
  const [r, g, b] = c.map(lin) as Rgb;
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
function fromLab([L, a, b]: [number, number, number]): Rgb {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [gam(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), gam(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s), gam(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)].map((v) => Math.round(Math.min(255, Math.max(0, v)))) as Rgb;
}
/** `t` of the way from `a` to `b`, in OKLab. */
export const mixLab = (a: Rgb, b: Rgb, t: number): Rgb => {
  const [x, y] = [toLab(a), toLab(b)];
  return fromLab([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t]);
};

/** Moves `c` toward `toward` in OKLab until it reaches `ratio`:1 on `ground`; returns the first that does, or the end. */
export function guard(c: Rgb, ground: Rgb, toward: Rgb, ratio = 4.5): Rgb {
  for (let t = 0; t <= 1.0001; t += 0.02) {
    const m = mixLab(c, toward, t);
    if (contrast(m, ground) >= ratio) return m;
  }
  return toward;
}

/** The CSS variables a palette sets on the root for a scheme (the values `tokens.json` would have). */
export function tokensFor(p: Palette, scheme: "light" | "dark"): Record<string, string> {
  const paper = hex(p.paper), ink = hex(p.ink);
  const ink2 = mixLab(paper, ink, 0.82), ink3 = guard(mixLab(paper, ink, 0.7), paper, scheme === "light" ? [0, 0, 0] : [255, 255, 255]), ink4 = mixLab(paper, ink, 0.54);
  const rgb = (c: Rgb) => c.join(" ");
  const text = (c: string) => toHex(guard(hex(c), paper, scheme === "light" ? [0, 0, 0] : [255, 255, 255]));
  const accent = hex(p.accent);
  return {
    paper: p.paper,
    tint: rgb(mixLab(paper, scheme === "light" ? [255, 255, 255] : [0, 0, 0], scheme === "light" ? 0.35 : 0.2)),
    ink: p.ink,
    "ink-rgb": rgb(ink),
    "shadow-rgb": scheme === "light" ? rgb(mixLab(ink, [0, 0, 0], 0.5)) : "0 0 0",
    "ink-2": toHex(ink2),
    "ink-3": toHex(ink3),
    "ink-4": toHex(ink4),
    "accent-base": p.accent,
    accent: text(p.accent),
    "accent-soft": `color-mix(in oklab, ${p.accent} ${scheme === "light" ? 14 : 22}%, transparent)`,
    danger: text(p.danger),
    good: text(p.good),
    warn: text(p.warn),
    info: text(p.info),
    ...(contrast([255, 255, 255], accent) < 4.5 ? { "accent-fill-ink": "#1f1f1f" } : {}),
  };
}

const KEYS = ["paper", "tint", "ink", "ink-rgb", "shadow-rgb", "ink-2", "ink-3", "ink-4", "accent-base", "accent", "accent-soft", "danger", "good", "warn", "info", "accent-fill-ink"];

/** Applies a theme's palette for the scheme on the root (inline variables win over the token sheet), or clears every override when `theme` is null. */
export function applyTheme(theme: Theme | null, scheme: "light" | "dark"): void {
  const st = document.documentElement.style;
  for (const k of KEYS) st.removeProperty(`--${k}`);
  if (!theme) return;
  for (const [k, v] of Object.entries(tokensFor(theme[scheme], scheme))) st.setProperty(`--${k}`, v);
}
