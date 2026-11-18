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
 *   - The card's veil is the computed floor plus a small margin, not a fixed strength.
 * Cost: a sort of at most a few thousand numbers per probe, about 60 probes per surface; it runs when the wallpaper, scheme, boxes or window size change.
 */
export type RGB = [number, number, number];
/** The colour scheme a veil is solved for. */
export type Scheme = "light" | "dark";
/** Which surface a veil sits on: `side` (a sidebar, straight on the wallpaper) or `card` (the middle card, which never flips its ink). */
export type Role = "side" | "card";
/** Which parts of the glass material are on (Appearance, Materials): `blur` frosts the picture, `dim` adds a neutral veil, `tint` a veil in the scheme's paper. All off leaves the bare picture. */
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

const lin = (v: number): number => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const delin = (l: number): number => Math.round(255 * (l <= 0.0031308 ? 12.92 * l : 1.055 * l ** (1 / 2.4) - 0.055));
/** WCAG relative luminance of an encoded colour. */
export const lum = ([r, g, b]: RGB): number => 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
/** WCAG contrast ratio of two luminances. */
export const ratio = (a: number, b: number): number => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
/** `a` of `v` over `w`, in encoded sRGB. */
export const over = (w: RGB, v: RGB, a: number): RGB => [(1 - a) * w[0] + a * v[0], (1 - a) * w[1] + a * v[1], (1 - a) * w[2] + a * v[2]];
const mix = (x: RGB, y: RGB, t: number): RGB => over(x, y, t);
/** The 1st percentile of a list. */
export const p01 = (xs: number[]): number => [...xs].sort((a, b) => a - b)[Math.floor(0.01 * (xs.length - 1))]!;

/** The opacity of the tint layer per surface when `tint` is on (a starting point; the solver raises the total until the text reaches `TARGET`). */
export const TINT_A: Record<Role, number> = { side: 0.35, card: 0.85 };
/** The opacity of the neutral dim layer per surface when `dim` is on (see `TINT_A`). */
export const DIM_A: Record<Role, number> = { side: 0.25, card: 0.6 };
const GAIN: Record<Scheme, number> = { light: 0.25, dark: 0.33 };
const C_CAP = 0.05, A_MAX = 0.9, MARGIN = 0.02, FLIP_GAIN = 0.12;
/** Headroom over each target for what the model does not see: the grain, the JPEG, and pixels between the 48 by 48 samples. */
const HEAD = 0.35;
/** The contrast ratios the veil is solved to for ink, secondary and tertiary text. Mutated by `setFloor`; read by every solve, so a change needs a re-solve (the wallpaper module schedules it). */
export const TARGET: { ink: number; ink2: number; ink3: number } = { ink: 7, ink2: 5.5, ink3: 4.5 };
/** The readability floors the veil is solved to: `current` (7, 5.5, 4.5) or `low` (4.5, 4, 3), which shows more of the picture. `low` exists for the owner's side-by-side comparison only (Settings, Appearance, Compare). */
const FLOORS = { current: { ink: 7, ink2: 5.5, ink3: 4.5 }, low: { ink: 4.5, ink2: 4, ink3: 3 } } as const;
/** Switches the readability floor between `current` and `low` (the owner's side-by-side comparison; nothing else should call it). Takes effect at the next solve. */
export function setFloor(f: keyof typeof FLOORS): void { Object.assign(TARGET, FLOORS[f]); }

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
  const composite = (w: RGB, aT: number, aD: number): RGB => over(over(w, tintRgb, aT), T.paper, aD);
  const worst = (aT: number, aD: number): number => p01(px.map((w) => ratio(lum(composite(w, aT, aD)), lum(T.ink))));
  let aT: number, aD: number;
  if (!flags.tint && !flags.dim) {
    // Nothing is on: an opaque surface, the one fallback that is always readable.
    aT = 0; aD = 1;
  } else if (role === "card") {
    // The card's veil is the floor plus a margin, split between the layers that are on.
    const total = (v: number) => (flags.tint && flags.dim ? [1 - Math.sqrt(1 - v), 1 - Math.sqrt(1 - v)] : flags.tint ? [v, 0] : [0, v]) as [number, number];
    const t = smallest((v) => worst(...total(Math.min(A_MAX, v))) >= TARGET.ink + HEAD);
    const v = Math.min(A_MAX, t + MARGIN);
    [aT, aD] = total(v);
  } else {
    aT = flags.tint ? TINT_A[role] : 0;
    aD = flags.dim ? DIM_A[role] : 0;
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
  const toward = (tok: RGB, target: number): RGB => mix(tok, T.ink, smallest((t) => p01(ground.map((g) => ratio(lum(g), lum(mix(tok, T.ink, t))))) >= target));
  return {
    scheme, tint: { a: aT, rgb: tintRgb }, dim: { a: aD, rgb: T.paper },
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
  if (role !== "side" || !other || (!flags.tint && !flags.dim)) return here;
  const flipped = solveOne(scheme === "light" ? "dark" : "light", role, flags, px, hue, other);
  return flipped.total < here.total - FLIP_GAIN ? flipped : here;
}

/** The contrast of `ink` on the pixels `px` under a veil (for tests and the report). */
export function contrastOn(px: RGB[], ink: RGB, v: Veil): { p01: number; worst: number } {
  const rs = px.map((w) => ratio(lum(over(over(w, v.tint.rgb, v.tint.a), v.dim.rgb, v.dim.a)), lum(ink)));
  return { p01: p01(rs), worst: Math.min(...rs) };
}
