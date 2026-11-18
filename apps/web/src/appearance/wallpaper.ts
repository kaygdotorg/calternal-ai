/**
 * The wallpaper pipeline, after the owner's calternal-ui plugin (`frame/preblur.ts`, `color.ts`, `wallpaper.ts`).
 *
 * A live `backdrop-filter` or `filter: blur()` over a viewport-sized picture is re-rasterised whenever the layer repaints. A blurred picture has no
 * fine detail, so it is rendered here once per (wallpaper, blur, viewport) into a small canvas (1/k of the viewport, k growing with the blur, up to 1/8),
 * the blur and a saturation boost baked in, exported as a JPEG blob and shown as a plain background. Upscaling it is itself a soft blur. Two copies
 * are made: the sidebars' (light blur) and the middle card's (heavy blur). From the sidebars' copy the same pass reads, once:
 *   - a brightness grid (so surfaces know how bright it is behind them: `setBackdropField`, no pixels read later),
 *   - the average tone and the most vivid hue (for the accent),
 *   - the pixels under each surface (the two sidebars and the card), from which `veil.ts` solves how much veil the text needs (`updateVeil`).
 * Cost: two canvas draws and one 24 by 14 read per change of wallpaper, blur or window size (resize is debounced 400 ms); nothing per frame.
 */
import { cssPx, luminance, scheduleMaterial, setBackdropField, currentScheme, type Field } from "@calternal/ui";
import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
import { surfaces } from "./surfaces.svelte";
import { solve, type Flags, type Ink, type RGB, type Role, type Scheme, type Veil } from "./veil";

type Layer = "side" | "card";
const SATURATE: Record<Layer, number> = { side: 120, card: 140 };
const GRID = { cols: 24, rows: 14 };

/** How the wallpaper is applied: blur radii (px) of the two copies, as chosen in settings. */
export interface WallpaperLook {
  /** Blur radius in px of the copy of the picture behind the sidebars (set in Appearance). */
  sideBlur: number;
  /** Blur radius in px of the copy behind the middle card (set in Appearance). */
  cardBlur: number;
}

/** What one reading of a wallpaper produced. */
export interface WallpaperColours {
  /** Average colour, `rgb(r g b)`. */
  tone: string;
  /** Most vivid common hue as OKLCH chroma and hue in degrees; null for a grey picture. */
  hue: { c: number; h: number } | null;
}

const root = () => document.documentElement.style;
let source: string | null = null;
let look: WallpaperLook = { sideBlur: 12, cardBlur: 75 };
const urls: Partial<Record<Layer, string>> = {};
let seq = 0;
let timer = 0;
let image: { src: string; img: Promise<HTMLImageElement> } | null = null;
const listeners = new Set<(c: WallpaperColours | null) => void>();

/** Subscribes to the colours read from each new wallpaper (the accent follows them). */
export function onWallpaperColours(fn: (c: WallpaperColours | null) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function load(src: string): Promise<HTMLImageElement> {
  if (image?.src === src) return image.img;
  const img = new Image();
  img.decoding = "async";
  const done = new Promise<HTMLImageElement>((resolve, reject) => {
    img.onload = () => resolve(img);
    img.onerror = reject;
  });
  img.src = src;
  image = { src, img: done };
  return done;
}

/** Renders the picture cover-fitted to the viewport, blurred and saturated, into a small canvas. */
function render(img: HTMLImageElement, blur: number, saturate: number): HTMLCanvasElement {
  // At most 1/4 scale: real Safari upscales a smaller copy without smoothing it, which showed as blocks.
  const k = Math.min(4, Math.max(1, blur / 8));
  const w = Math.max(16, Math.round(innerWidth / k));
  const h = Math.max(16, Math.round(innerHeight / k));
  const r = blur / k;
  const m = Math.ceil(2 * r); // draw past the edges so the blur never fades them to transparent
  const scale = Math.max((w + 2 * m) / img.naturalWidth, (h + 2 * m) / img.naturalHeight);
  const dw = img.naturalWidth * scale;
  const dh = img.naturalHeight * scale;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d")!;
  ctx.filter = `blur(${r}px) saturate(${saturate}%)`;
  ctx.drawImage(img, (w - dw) / 2, (h - dh) / 2, dw, dh);
  return canvas;
}

const toUrl = (c: HTMLCanvasElement): Promise<string> =>
  new Promise((resolve, reject) => c.toBlob((b) => (b ? resolve(URL.createObjectURL(b)) : reject(new Error("canvas export failed"))), "image/jpeg", 0.9));

/** sRGB (0..255) to OKLCH chroma and hue (lightness is not needed). */
function toOklch(r: number, g: number, b: number): { c: number; h: number } {
  const lin = (v: number) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const [R, G, B] = [lin(r), lin(g), lin(b)];
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const b2 = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { c: Math.hypot(a, b2), h: ((Math.atan2(b2, a) * 180) / Math.PI + 360) % 360 };
}

/** Reads the average tone and the most vivid hue from a 48 by 48 copy. Hues are binned by 15 degrees and scored by count times chroma. */
function colours(img: HTMLImageElement): WallpaperColours {
  const size = 48;
  const cv = document.createElement("canvas");
  cv.width = cv.height = size;
  const ctx = cv.getContext("2d", { willReadFrequently: true })!;
  ctx.drawImage(img, 0, 0, size, size);
  const d = ctx.getImageData(0, 0, size, size).data;
  let r = 0, g = 0, b = 0;
  const bins = Array.from({ length: 24 }, () => ({ score: 0, x: 0, y: 0, c: 0, n: 0 }));
  for (let i = 0; i < d.length; i += 4) {
    r += d[i]!; g += d[i + 1]!; b += d[i + 2]!;
    const { c, h } = toOklch(d[i]!, d[i + 1]!, d[i + 2]!);
    if (c < 0.03) continue;
    const bin = bins[Math.floor(h / 15) % 24]!;
    bin.score += c; bin.c += c; bin.n += 1;
    bin.x += Math.cos((h * Math.PI) / 180) * c;
    bin.y += Math.sin((h * Math.PI) / 180) * c;
  }
  const n = d.length / 4;
  const best = bins.reduce((a, z) => (z.score > a.score ? z : a));
  return {
    tone: `rgb(${Math.round(r / n)} ${Math.round(g / n)} ${Math.round(b / n)})`,
    hue: best.n < 20 ? null : { c: best.c / best.n, h: ((Math.atan2(best.y, best.x) * 180) / Math.PI + 360) % 360 },
  };
}

/** The brightness grid of a blurred copy: each cell's relative luminance. */
function fieldOf(c: HTMLCanvasElement): Field {
  const g = document.createElement("canvas");
  g.width = GRID.cols;
  g.height = GRID.rows;
  const ctx = g.getContext("2d", { willReadFrequently: true })!;
  ctx.drawImage(c, 0, 0, GRID.cols, GRID.rows);
  const d = ctx.getImageData(0, 0, GRID.cols, GRID.rows).data;
  const lum = new Float32Array(GRID.cols * GRID.rows);
  for (let i = 0; i < lum.length; i++) lum[i] = luminance(`#${[0, 1, 2].map((k) => d[i * 4 + k]!.toString(16).padStart(2, "0")).join("")}`);
  return { cols: GRID.cols, rows: GRID.rows, lum };
}

type Frac = { x0: number; y0: number; x1: number; y1: number };

/** The surfaces' rectangles as fractions of the viewport. On a narrow screen one sidebar fills it; the card is the viewport. */
function rects(): { left: Frac; right: Frac; card: Frac } {
  const w = innerWidth || 1, h = innerHeight || 1;
  if (w < 760) { const all = { x0: 0, y0: 0, x1: 1, y1: 1 }; return { left: all, right: all, card: all }; }
  const side = cssPx("--side-w", 240) / w;
  const right = cssPx("--right-w", 340) / w;
  const c = document.querySelector(".mid-card")?.getBoundingClientRect();
  const card = c && c.width > 0 ? { x0: c.left / w, y0: c.top / h, x1: c.right / w, y1: c.bottom / h } : { x0: side, y0: 0, x1: 1 - right, y1: 1 };
  return { left: { x0: 0, y0: 0, x1: side, y1: 1 }, right: { x0: 1 - right, y0: 0, x1: 1, y1: 1 }, card };
}

const SAMPLE = 48; // at most 48 by 48 pixels per surface
/** The pixels of a copy inside a rectangle (fractions of the viewport), as encoded RGB. */
function sample(c: HTMLCanvasElement, r: Frac): RGB[] {
  const cv = document.createElement("canvas");
  cv.width = cv.height = SAMPLE;
  const ctx = cv.getContext("2d", { willReadFrequently: true })!;
  const sx = r.x0 * c.width, sy = r.y0 * c.height;
  ctx.drawImage(c, sx, sy, Math.max(1, (r.x1 - r.x0) * c.width), Math.max(1, (r.y1 - r.y0) * c.height), 0, 0, SAMPLE, SAMPLE);
  const d = ctx.getImageData(0, 0, SAMPLE, SAMPLE).data;
  const out: RGB[] = [];
  for (let i = 0; i < d.length; i += 4) out.push([d[i]!, d[i + 1]!, d[i + 2]!]);
  return out;
}

const hex = (h: string): RGB => [parseInt(h.slice(1, 3), 16), parseInt(h.slice(3, 5), 16), parseInt(h.slice(5, 7), 16)];
/** A colour token as the browser resolves it (a theme may have replaced it), as encoded RGB; `fallback` when it cannot be read. */
function resolved(name: string, fallback: RGB): RGB {
  const e = document.createElement("span");
  e.style.cssText = `position:absolute;visibility:hidden;color:var(${name})`;
  document.body.append(e);
  const m = getComputedStyle(e).color.match(/[\d.]+/g);
  e.remove();
  return m && m.length >= 3 && m.slice(0, 3).every((v) => !Number.isNaN(parseFloat(v))) ? [Math.round(+m[0]!), Math.round(+m[1]!), Math.round(+m[2]!)] : fallback;
}
const inkOf = (s: Scheme): Ink => ({ ink: hex(tokens[s].ink), ink2: hex(tokens[s]["ink-2"]), ink3: hex(tokens[s]["ink-3"]), paper: hex(tokens[s].paper) });
const rgbCss = (c: RGB): string => `${Math.round(c[0])} ${Math.round(c[1])} ${Math.round(c[2])}`;

let copies: { side: HTMLCanvasElement; card: HTMLCanvasElement } | null = null;
/**
 * Solves the veil of each surface from the pixels under it and writes the result as CSS variables: for each of `l` (left sidebar), `r` (right sidebar) and
 * `c` (the card) the tint and dim layers (`--l-tint`, `--l-tint-a`, `--l-dim`, `--l-dim-a`), the surface's own secondary inks (`--l-ink2`, `--l-ink3`) and the
 * ready-made background `--veil-l`. Called when a wallpaper is read, and when the scheme, the three boxes or the window change. Reads: the two copies
 * already made (no decode); cost: see veil.ts.
 */
export function updateVeil(want: Flags): void {
  // Reduced transparency asks for opaque surfaces: the same fallback as having every box off.
  const flags: Flags = matchMedia("(prefers-reduced-transparency: reduce)").matches ? { blur: false, dim: false, tint: false } : want;
  const scheme = currentScheme();
  const T: Ink = { ink: resolved("--ink", inkOf(scheme).ink), ink2: resolved("--ink-2", inkOf(scheme).ink2), ink3: resolved("--ink-3", inkOf(scheme).ink3), paper: resolved("--paper", inkOf(scheme).paper) };
  const other = inkOf(scheme === "light" ? "dark" : "light");
  const r = rects();
  const flat = (): RGB[] => [resolved("--backdrop", inkOf(scheme).paper)];
  const px = (c: HTMLCanvasElement | undefined, f: Frac): RGB[] => (source && c ? sample(c, f) : flat());
  const jobs: [string, Role, RGB[]][] = [["l", "side", px(copies?.side, r.left)], ["r", "side", px(copies?.side, r.right)], ["c", "card", px(copies?.card, r.card)]];
  const out: Record<string, Veil> = {};
  for (const [k, role, pixels] of jobs) {
    const v: Veil = craftLight && scheme === "light" ? { scheme: "light", tint: { a: 0.92, rgb: [255, 255, 255] }, dim: { a: 0, rgb: [0, 0, 0] }, ink2: T.ink2, ink3: T.ink3, total: 0.92, ratio: 99 } : solve(scheme, role, flags, pixels, T, other);
    if (veilScale !== 1 && !(craftLight && scheme === "light")) { v.tint = { ...v.tint, a: Math.min(0.92, v.tint.a * veilScale) }; v.dim = { ...v.dim, a: Math.min(0.92, v.dim.a * veilScale) }; }
    out[k] = v;
    const ink2 = v.scheme === scheme ? v.ink2 : v.ink2, ink3 = v.ink3;
    root().setProperty(`--${k}-tint`, rgbCss(v.tint.rgb));
    root().setProperty(`--${k}-tint-a`, v.tint.a.toFixed(3));
    root().setProperty(`--${k}-dim`, rgbCss(v.dim.rgb));
    root().setProperty(`--${k}-dim-a`, v.dim.a.toFixed(3));
    root().setProperty(`--${k}-ink2`, `rgb(${rgbCss(ink2)})`);
    root().setProperty(`--${k}-ink3`, `rgb(${rgbCss(ink3)})`);
    root().setProperty(`--veil-${k}`, `linear-gradient(rgb(var(--${k}-dim) / var(--${k}-dim-a)) 0 0), linear-gradient(rgb(var(--${k}-tint) / var(--${k}-tint-a)) 0 0)`);
  }
  const flip = (v: Veil | undefined) => (v && v.scheme !== scheme ? v.scheme : null);
  surfaces.left = flip(out.l);
  surfaces.right = flip(out.r);
  scheduleMaterial();
}

function clear(): void {
  for (const l of ["side", "card"] as const) {
    root().removeProperty(`--wall-${l}`);
    if (urls[l]) URL.revokeObjectURL(urls[l]!);
    delete urls[l];
  }
  copies = null;
  setBackdropField(null);
  updateVeil(flagsNow);
  for (const fn of listeners) fn(null);
}

async function update(): Promise<void> {
  timer = 0;
  const mine = ++seq;
  if (!source) return clear();
  try {
    const img = await load(source);
    const side = render(img, look.sideBlur, SATURATE.side);
    const card = render(img, look.cardBlur, SATURATE.card);
    const [su, cu] = await Promise.all([toUrl(side), toUrl(card)]);
    if (mine !== seq) {
      URL.revokeObjectURL(su);
      URL.revokeObjectURL(cu);
      return;
    }
    for (const [l, u] of [["side", su], ["card", cu]] as const) {
      const old = urls[l];
      urls[l] = u;
      root().setProperty(`--wall-${l}`, `url("${u}")`);
      if (old) URL.revokeObjectURL(old);
    }
    copies = { side, card };
    setBackdropField(fieldOf(side));
    updateVeil(flagsNow);
    const c = colours(img);
    for (const fn of listeners) fn(c);
  } catch {
    clear(); // an unreadable picture: back to the mesh
  }
}

function schedule(ms = 60): void {
  clearTimeout(timer);
  timer = window.setTimeout(update, ms);
}

/** Shows a wallpaper (an image URL: a blob, a data URL or a same-origin file) or none. The blur is done here, once. */
let flagsNow: Flags = { blur: true, dim: true, tint: true };
/** The three boxes as currently chosen; the veil is solved with them whenever the pictures are re-read. */
/** The comparison of the owner's: in the light scheme, every surface is Craft's nearly opaque white (0.92) over the blurred, unchanged picture instead of the solved veil. Temporary. */
/** Clear to tinted: the solved veil's strength is scaled by this (1 leaves it as solved; below 1 shows more wallpaper and may fall under the readability floor; above 1 is denser). */
let veilScale = 1;
/** Sets the clear-to-tinted position, 0 (clear) to 1 (tinted); 0.5 is the solved veil. Temporary, with the comparison. */
export function setClarity(t: number): void { veilScale = t <= 0.5 ? 0.3 + 1.4 * t : 1 + 1.6 * (t - 0.5); }
let craftLight = false;
/** Chooses the "Craft" light-mode surface (the picture under a nearly opaque white) over the default solved veil; the comparison control in Appearance. Read by the next veil solve. */
export function setCraftLight(b: boolean): void { craftLight = b; }
/** Records which material layers are on. It does not solve anything itself: the veil is solved with these flags the next time the pictures are read (a new wallpaper, a scheme change, a resize). */
export function setFlags(f: Flags): void {
  flagsNow = f;
}

/** Makes `src` (a URL, or null for none) the wallpaper, with its precomputed `look` when it has one, and schedules the work that follows: the blurred copies and the veil solve. The same source with no new look is a no-op. */
export function setWallpaper(src: string | null, l?: WallpaperLook): void {
  if (l) look = l;
  if (src === source && !l) return;
  source = src;
  schedule();
}

let resizeTimer = 0;
addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => source && schedule(0), 400); // a bigger window would upscale the small copy further
}, { passive: true });
