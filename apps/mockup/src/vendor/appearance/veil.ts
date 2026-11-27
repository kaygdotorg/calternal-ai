/**
 * The veil function (docs/ui-craft-and-settings-study.md sections 5 and 6): for one surface, how much of the scheme's paper colour must lie over the
 * wallpaper under it for text to be readable, solved from the surface's own pixels rather than from a brightness estimate.
 *
 *   - Primary ink must reach 7:1 on the worst 1 % of the pixels under the surface; ink-2 and ink-3 are moved toward the primary, per surface, until they
 *     reach 5.5:1 and 4.5:1.
 *   - Compositing is done in gamma-encoded sRGB, as CSS does it (a layer of alpha a over w is (1 - a) * w + a * layer, per encoded channel).
 *   - Two layers sit over the picture: Tint (the paper colour carrying a whisper of the picture's hue) then Dim (the plain paper colour). Blur is not a
 *     layer: it is decided before pixels are read (the pre-blurred copy). With a box off, the others are raised until the target is met; with all of them
 *     off the surface is opaque paper.
 *   - A sidebar may flip its text between light and dark when that needs clearly less veil (as Craft's header does); the card never does.
 *   - The card starts much denser than a sidebar (tint 0.85 and dim 0.3 over the heavily blurred copy: the defaults of the calternal-ui plugin's chat surface, about 0.9 in all; a sidebar at 0.35 and no dim, raised only as the floor needs), so the middle
 *     card reads as a calm sheet and the sidebars as the wallpaper seen through glass; the readability floor can only raise it. docs/ui-design-system.md has the numbers.
 *   - A sidebar flips from the light scheme to dark text over a very dark picture; in the dark scheme nothing flips (a light veil in dark mode is never right).
 * Cost: a sort of at most a few thousand numbers per probe, about 60 probes per surface; it runs when the wallpaper, scheme, boxes or window size change.
 */
export type RGB = [number, number, number];
/** The colour scheme a veil is solved for. */
export type Scheme = "light" | "dark";
/** Which surface a veil sits on: `side` (a sidebar, straight on the wallpaper) or `card` (the middle card, which never flips its ink). */
export type Role = "side" | "card";
/** Which parts of the glass material are on (Appearance, Materials): `blur` frosts the picture, `dim` adds a neutral veil, `tint` a veil in the scheme's paper. With tint and dim both off the surface is opaque paper, which is always readable. */
export interface Flags { blur: boolean; dim: boolean; tint: boolean }
/** The colours of one scheme: primary, secondary and tertiary ink, and the paper the veil is made of. */
export interface Ink { ink: RGB; ink2: RGB; ink3: RGB; paper: RGB }
/** What the solver decided for one surface. */
export interface Veil {
  /** Which scheme's ink the surface uses (a sidebar may differ from the page's scheme). */
  scheme: Scheme;
  /** The tint layer: its opacity and colour. */
  tint: { a: number; rgb: RGB };
  /** The neutral dim layer: its opacity and colour. */
  dim: { a: number; rgb: RGB };
  /** The secondary text colour the solver chose to meet its contrast target. */
  ink2: RGB;
  /** The tertiary text colour the solver chose to meet its contrast target. */
  ink3: RGB;
  /** 1 - (1 - tint)(1 - dim): how much of the picture is covered. */
  total: number;
  /** Worst-1 % contrast of primary ink on the result. */
  ratio: number;
}

/** sRGB to linear light: a table for whole channel values (the blends that are measured are rounded to them), the formula between. */
const LIN = new Float64Array(256).map((_, i) => { const v = i / 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; });
const lin = (v: number): number => (Number.isInteger(v) && v >= 0 && v <= 255 ? LIN[v]! : (v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const delin = (l: number): number => Math.round(255 * (l <= 0.0031308 ? 12.92 * l : 1.055 * l ** (1 / 2.4) - 0.055));
/** WCAG relative luminance of a colour given as three whole channel values (0 to 255): no allocation, for the loops that measure many pixels. */
export const lumInt = (r: number, g: number, b: number): number => 0.2126 * LIN[r]! + 0.7152 * LIN[g]! + 0.0722 * LIN[b]!;
/** WCAG relative luminance of an encoded colour. */
export const lum = ([r, g, b]: RGB): number => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
/** WCAG contrast ratio of two luminances. */
export const ratio = (a: number, b: number): number => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
/** `a` of `v` over `w`, in encoded sRGB. */
export const over = (w: RGB, v: RGB, a: number): RGB => [(1 - a) * w[0] + a * v[0], (1 - a) * w[1] + a * v[1], (1 - a) * w[2] + a * v[2]];
const mix = (x: RGB, y: RGB, t: number): RGB => over(x, y, t);
/** The 1st percentile of a list: the element a full sort would put at index `floor(0.01 * (n - 1))`, found by selection (linear time; the sort it replaces was most of the cost of solving a veil, repeated for every surface on the page). */
export function p01(xs: number[]): number {
  const k = Math.floor(0.01 * (xs.length - 1)), a = Float64Array.from(xs);
  let lo = 0, hi = a.length - 1;
  while (lo < hi) {
    const pivot = a[(lo + hi) >> 1]!; let i = lo, j = hi;
    while (i <= j) { while (a[i]! < pivot) i++; while (a[j]! > pivot) j--; if (i <= j) { const t = a[i]!; a[i] = a[j]!; a[j] = t; i++; j--; } }
    if (k <= j) hi = j; else if (k >= i) lo = i; else break;
  }
  return a[k]!;
}

/** The starting opacity of a surface's tint layer when `tint` is on (a sidebar's 0.35 is the plugin's sidebar default; the card's 0.85 its chat default); the solver raises Dim (or Tint when Dim is off) above the starting values only as far as the readability floor needs. The card's veil is computed, not started from here. */
export const TINT_A: Record<Role, number> = { side: 0.35, card: 0.85 };
/** The starting opacity of a sidebar's neutral dim layer when `dim` is on (see `TINT_A`). */
export const DIM_A: Record<Role, number> = { side: 0, card: 0.3 };
/** The card's Dim in the dark scheme: deeper than the plugin's 0.3, so the card stays clearly darker than the sidebars (which the readability floor keeps dim over the default photograph); docs/ui-design-system.md has the measurement. */
export const CARD_DIM_DARK = 0.45;
const GAIN: Record<Scheme, number> = { light: 0.25, dark: 0.33 };
const C_CAP = 0.05, A_MAX = 0.9, FLIP_GAIN = 0.12;
/** Headroom over each target for what the model does not see: the grain, the JPEG, and pixels between the 48 by 48 samples. */
const HEAD = 2.2; // mockup: 0.35 in the design system; raised so the worst pixel of a real render (grain, JPEG, the pixels between the 48 by 48 samples) still clears the floor
/** The contrast ratios the veil is solved to for ink, secondary and tertiary text: the one readability floor (7:1, with text that flips between light and dark where it must; the owner's decision, 2026-11-18). */
export const TARGET: { readonly ink: number; readonly ink2: number; readonly ink3: number } = { ink: 7, ink2: 6.2, ink3: 5.2 }; // the mockup solves for the worst pixel, not the worst 1 %, so the secondary inks carry 0.7 more than the design system's 5.5 and 4.5 (an independent review measures the worst pixel)

// ---- OKLCH, for the veil's hue ----
const toOk = ([r, g, b]: RGB): { L: number; c: number; h: number } => {
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const b2 = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, c: Math.hypot(a, b2), h: ((Math.atan2(b2, a) * 180) / Math.PI + 360) % 360 };
};
const fromOk = (L: number, c: number, h: number): { rgb: RGB; inGamut: boolean } => {
  const a = c * Math.cos((h * Math.PI) / 180), b = c * Math.sin((h * Math.PI) / 180);
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const R = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s, G = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s, B = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
  const ok = [R, G, B].every((v) => v >= -0.0005 && v <= 1.0005);
  const cl = (v: number) => delin(Math.min(1, Math.max(0, v)));
  return { rgb: [cl(R), cl(G), cl(B)], inGamut: ok };
};
/** The paper colour with chroma `c` at hue `h`, reduced until it fits sRGB at the paper's own lightness. */
export function paperWithChroma(paper: RGB, c: number, h: number): RGB {
  const { L } = toOk(paper);
  for (let k = c; k > 0.0005; k *= 0.85) {
    const r = fromOk(L, k, h);
    if (r.inGamut) return r.rgb;
  }
  return paper;
}
/** Mean OKLCH chroma and hue of a set of pixels (the hue of the mean colour). */
export function meanOklch(px: RGB[]): { c: number; h: number } {
  let r = 0, g = 0, b = 0;
  for (const p of px) { r += p[0]; g += p[1]; b += p[2]; }
  const n = Math.max(1, px.length);
  const { c, h } = toOk([r / n, g / n, b / n]);
  return { c, h };
}

/** The smallest t in [0, 1] for which `ok(t)` holds (ok is monotone), by bisection; 1 when it never does. */
function smallest(ok: (t: number) => boolean): number {
  if (ok(0)) return 0;
  let lo = 0, hi = 1;
  for (let i = 0; i < 18; i++) { const m = (lo + hi) / 2; if (ok(m)) hi = m; else lo = m; }
  return hi;
}

function solveOne(scheme: Scheme, role: Role, flags: Flags, px: RGB[], hue: { c: number; h: number }, T: Ink): Veil {
  const tintRgb = paperWithChroma(T.paper, Math.min(GAIN[scheme] * hue.c, C_CAP), hue.h);
  // The dim layer deepens a surface toward its scheme: black over a dark card, white over a light one (the plugin's rule); a sidebar's is the plain paper.
  const dimRgb: RGB = role === "card" ? (scheme === "dark" ? [0, 0, 0] : [255, 255, 255]) : T.paper;
  const composite = (w: RGB, aT: number, aD: number): RGB => over(over(w, tintRgb, aT), dimRgb, aD);
  const lumInk = lum(T.ink);
  const worst = (aT: number, aD: number): number => p01(px.map((w) => ratio(lum(composite(w, aT, aD)), lumInk)));
  let aT: number, aD: number;
  if (!flags.tint && !flags.dim) {
    // Nothing is on: an opaque surface, the one fallback that is always readable.
    aT = 0; aD = 1;
  } else {
    aT = flags.tint ? TINT_A[role] : 0;
    aD = flags.dim ? (role === "card" && scheme === "dark" ? CARD_DIM_DARK : DIM_A[role]) : 0;
    if (worst(aT, aD) < TARGET.ink + HEAD) {
      // The readability floor: raise Dim (or Tint, when Dim is off) until the target is met, never above A_MAX.
      const raiseDim = flags.dim || !flags.tint;
      const ok = (a: number) => worst(raiseDim ? aT : a, raiseDim ? a : aD) >= TARGET.ink + HEAD;
      let lo = raiseDim ? aD : aT, hi = A_MAX;
      if (!ok(hi)) lo = hi;
      else for (let i = 0; i < 18; i++) { const m = (lo + hi) / 2; if (ok(m)) hi = m; else lo = m; }
      if (raiseDim) aD = hi; else aT = hi;
    }
  }
  const ground = px.map((w) => composite(w, aT, aD));
  // The ground does not change while the ink is searched for: its luminances are read once, and each candidate's once per step (not once per pixel per step: that was most of the cost of a first Clear to tinted step).
  const lumGround = ground.map((g) => lum(g));
  const toward = (tok: RGB, target: number): RGB => mix(tok, T.ink, smallest((t) => { const l = lum(mix(tok, T.ink, t)); return p01(lumGround.map((g) => ratio(g, l))) >= target; }));
  return {
    scheme, tint: { a: aT, rgb: tintRgb }, dim: { a: aD, rgb: dimRgb },
    ink2: toward(T.ink2, TARGET.ink2 + HEAD), ink3: toward(T.ink3, TARGET.ink3 + HEAD),
    total: 1 - (1 - aT) * (1 - aD), ratio: worst(aT, aD),
  };
}

/**
 * Solves one surface. `px` are the encoded pixels of the picture copy that will show under it (pre-blurred when Blur is on); `T` holds the current
 * scheme's colours and `other` the opposite scheme's, used only for a sidebar's possible flip.
 */
export function solve(scheme: Scheme, role: Role, flags: Flags, px: RGB[], T: Ink, other?: Ink): Veil {
  const hue = meanOklch(px);
  const here = solveOne(scheme, role, flags, px, hue, T);
  if (role !== "side" || scheme !== "light" || !other || (!flags.tint && !flags.dim)) return here;
  const flipped = solveOne(scheme === "light" ? "dark" : "light", role, flags, px, hue, other);
  return flipped.total < here.total - FLIP_GAIN ? flipped : here;
}

/** The least veil Clear to tinted leaves on a surface: Clear shows more of the picture but never turns a dark surface light or a light one dark, so the scheme's own paper always covers this much. */
export const MIN_VEIL = 0.3;
/**
 * The veil with its strength multiplied by `k` (Clear to tinted: below 1 shows more picture, above 1 less), each layer capped at 0.92 and the whole never under `MIN_VEIL`.
 * The colours stay the veil's own (the scheme's paper), so no setting of the slider can tint a dark surface with a light colour.
 */
export function scaled(v: Veil, k: number): Veil {
  if (k === 1) return v;
  const t = Math.min(0.92, v.tint.a * k);
  let d = Math.min(0.92, v.dim.a * k);
  if (1 - (1 - t) * (1 - d) < MIN_VEIL) d = Math.min(0.92, 1 - (1 - MIN_VEIL) / (1 - t));
  return { ...v, tint: { ...v.tint, a: t }, dim: { ...v.dim, a: d }, total: 1 - (1 - t) * (1 - d) };
}

/** The contrast of `ink` on the pixels `px` under a veil (for tests and the report). */
export function contrastOn(px: RGB[], ink: RGB, v: Veil): { p01: number; worst: number } {
  const rs = px.map((w) => ratio(lum(over(over(w, v.tint.rgb, v.tint.a), v.dim.rgb, v.dim.a)), lum(ink)));
  return { p01: p01(rs), worst: Math.min(...rs) };
}
