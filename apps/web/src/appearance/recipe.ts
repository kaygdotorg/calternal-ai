/**
 * Generated backgrounds are recipes, not images (docs/ui-backgrounds-and-themes.md): a few colour points, a spread and a warp, about 100 bytes each.
 * One small deterministic renderer turns a recipe into a picture once; the wallpaper pipeline then pre-blurs it once like any photo. Colours are blended in
 * OKLab (never sRGB: no grey dead zone), the picture is bent by a gentle sine warp, and nothing animates. Low chroma and a short hue travel keep it
 * calm; grain is a runtime layer in the shell, not baked in here (lossy encoders erase baked grain).
 * Dark is the same recipe with lightness remapped, not a darkened copy. The renderer is pure maths plus one canvas wrapper, so a Rust port renders
 * the same pixels for the native clients.
 */
import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
import { oklchToRgb, type Rgb } from "./accent";

/** One colour point: position (0..1) and colour as OKLCH. */
export interface Point {
  x: number;
  y: number;
  L: number;
  C: number;
  h: number;
}

/** A generated look. `mesh` recipes carry fixed colours per scheme instead of points (the token mesh). */
export interface Recipe {
  id: string;
  name: string;
  points: Point[];
  /** Spread of each point as a fraction of the picture width. */
  sigma: number;
  /** Strength of the sine warp. */
  warp: number;
  seed: number;
}

const P = (x: number, y: number, L: number, C: number, h: number): Point => ({ x, y, L, C, h });

/** The built-in recipes: chroma at most about 0.06, hue travel under about 60 degrees. */
export const RECIPES: Recipe[] = [
  { id: "dusk", name: "Dusk", sigma: 0.46, warp: 0.05, seed: 3, points: [P(0.1, 0.15, 0.6, 0.05, 280), P(0.5, 0.05, 0.68, 0.045, 300), P(0.9, 0.2, 0.74, 0.055, 350), P(0.15, 0.85, 0.58, 0.05, 285), P(0.6, 0.75, 0.72, 0.05, 5), P(0.95, 0.95, 0.78, 0.05, 20)] },
  { id: "sage", name: "Sage", sigma: 0.46, warp: 0.04, seed: 7, points: [P(0.1, 0.1, 0.86, 0.03, 100), P(0.55, 0.15, 0.78, 0.04, 130), P(0.95, 0.1, 0.84, 0.03, 95), P(0.1, 0.9, 0.74, 0.045, 140), P(0.5, 0.8, 0.8, 0.035, 120), P(0.95, 0.9, 0.88, 0.025, 100)] },
  { id: "slate", name: "Slate", sigma: 0.5, warp: 0.03, seed: 11, points: [P(0.1, 0.2, 0.52, 0.03, 250), P(0.5, 0.1, 0.62, 0.028, 255), P(0.9, 0.2, 0.7, 0.025, 240), P(0.2, 0.85, 0.5, 0.03, 260), P(0.7, 0.8, 0.66, 0.03, 245)] },
  { id: "dune", name: "Dune", sigma: 0.45, warp: 0.05, seed: 13, points: [P(0.1, 0.2, 0.86, 0.045, 80), P(0.5, 0.1, 0.8, 0.055, 65), P(0.9, 0.25, 0.74, 0.06, 45), P(0.15, 0.85, 0.78, 0.05, 70), P(0.6, 0.8, 0.7, 0.06, 40), P(0.95, 0.9, 0.66, 0.055, 35)] },
  { id: "lagoon", name: "Lagoon", sigma: 0.46, warp: 0.05, seed: 17, points: [P(0.1, 0.15, 0.78, 0.045, 195), P(0.5, 0.1, 0.7, 0.055, 205), P(0.9, 0.2, 0.62, 0.06, 225), P(0.15, 0.85, 0.66, 0.05, 200), P(0.6, 0.75, 0.58, 0.058, 230), P(0.95, 0.9, 0.7, 0.045, 215)] },
  { id: "moss", name: "Moss", sigma: 0.46, warp: 0.04, seed: 19, points: [P(0.1, 0.1, 0.88, 0.03, 100), P(0.55, 0.2, 0.7, 0.05, 115), P(0.95, 0.15, 0.82, 0.035, 98), P(0.1, 0.9, 0.62, 0.05, 120), P(0.55, 0.8, 0.74, 0.04, 108)] },
  { id: "orchid", name: "Orchid", sigma: 0.46, warp: 0.05, seed: 23, points: [P(0.1, 0.15, 0.84, 0.04, 305), P(0.5, 0.1, 0.78, 0.05, 320), P(0.9, 0.2, 0.82, 0.05, 350), P(0.15, 0.85, 0.74, 0.055, 310), P(0.6, 0.8, 0.8, 0.05, 335), P(0.95, 0.9, 0.86, 0.04, 355)] },
];

const lin = (v: number) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
const gam = (v: number) => 255 * (v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055);
type Lab = [number, number, number];
const labOf = (c: Rgb): Lab => {
  const [r, g, b] = c.map(lin) as Rgb;
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b), m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b), s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
};
const rgbOf = ([L, a, b]: Lab): Rgb => {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3, m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3, s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [gam(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), gam(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s), gam(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)].map((v) => Math.round(Math.min(255, Math.max(0, v)))) as Rgb;
};
const hexRgb = (s: string): Rgb => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16)) as Rgb;

/** The remap that makes the dark rendition: lighter colours become dark ones, keeping their order, and chroma eases off. */
export const darken = (L: number, C: number): [number, number] => [0.17 + L * 0.3, C * 0.85];

/** The colour of a recipe at (u, v), both 0..1, for a scheme. Pure and deterministic. */
export function sample(r: Recipe, scheme: "light" | "dark", u: number, v: number): Rgb {
  const wu = u + r.warp * Math.sin(2 * Math.PI * (v * 1.3 + r.seed * 0.137));
  const wv = v + r.warp * Math.sin(2 * Math.PI * (u * 1.1 + r.seed * 0.241));
  let sum = 0, L = 0, a = 0, b = 0;
  for (const p of r.points) {
    const d = (wu - p.x) ** 2 + (wv - p.y) ** 2;
    const w = Math.exp(-d / (2 * r.sigma * r.sigma));
    const [l, c] = scheme === "dark" ? darken(p.L, p.C) : [p.L, p.C];
    const hr = (p.h * Math.PI) / 180;
    sum += w; L += w * l; a += w * c * Math.cos(hr); b += w * c * Math.sin(hr);
  }
  return rgbOf([L / sum, a / sum, b / sum]);
}

/** The quiet default, "haze": the shell's own token mesh (warm grey to cool grey) blended in OKLab, scheme by scheme. */
export function haze(scheme: "light" | "dark", u: number, v: number): Rgb {
  const mesh = tokens.backdrop[scheme].map((c) => labOf(hexRgb(c)));
  const gx = Math.min(Math.max(u * 2, 0), 1.999), gy = Math.min(Math.max(v * 2, 0), 1.999);
  const i = Math.floor(gx), j = Math.floor(gy), fx = gx - i, fy = gy - j;
  const at = (x: number, y: number) => mesh[y * 3 + x]!;
  const mixc = (k: number): number => at(i, j)[k]! * (1 - fx) * (1 - fy) + at(i + 1, j)[k]! * fx * (1 - fy) + at(i, j + 1)[k]! * (1 - fx) * fy + at(i + 1, j + 1)[k]! * fx * fy;
  return rgbOf([mixc(0), mixc(1), mixc(2)]);
}

/** Renders a recipe (or "haze") into a canvas of `w` by `h`, computed on a coarse grid and scaled up (it is blurred anyway), and exports a JPEG blob URL. */
export async function renderRecipe(id: string, scheme: "light" | "dark", w: number, h: number): Promise<string> {
  const r = RECIPES.find((x) => x.id === id);
  const gw = 192, gh = Math.round((192 * h) / w);
  const small = document.createElement("canvas");
  small.width = gw;
  small.height = gh;
  const ctx = small.getContext("2d")!;
  const img = ctx.createImageData(gw, gh);
  for (let y = 0; y < gh; y++) for (let x = 0; x < gw; x++) {
    const c = r ? sample(r, scheme, x / (gw - 1), y / (gh - 1)) : haze(scheme, x / (gw - 1), y / (gh - 1));
    const k = (y * gw + x) * 4;
    img.data[k] = c[0]; img.data[k + 1] = c[1]; img.data[k + 2] = c[2]; img.data[k + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
  const out = document.createElement("canvas");
  out.width = w;
  out.height = h;
  const o = out.getContext("2d")!;
  o.imageSmoothingQuality = "high";
  o.drawImage(small, 0, 0, w, h);
  return new Promise((res, rej) => out.toBlob((b) => (b ? res(URL.createObjectURL(b)) : rej(new Error("render failed"))), "image/jpeg", 0.92));
}

export { oklchToRgb };
