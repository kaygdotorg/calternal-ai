/**
 * The wallpaper pipeline, after the owner's calternal-ui plugin (`frame/preblur.ts`, `color.ts`, `wallpaper.ts`).
 *
 * A live `backdrop-filter` or `filter: blur()` over a viewport-sized picture is re-rasterised whenever the layer repaints. A blurred picture has no
 * fine detail, so it is rendered here once per (wallpaper, blur, viewport) into a small canvas (1/k of the viewport, k growing with the blur, up to 1/8),
 * the blur and a saturation boost baked in, exported as a JPEG blob and shown as a plain background. Upscaling it is itself a soft blur. Two copies
 * are made: the sidebars' (light blur) and the middle card's (heavy blur). From the sidebars' copy the same pass reads, once:
 *   - a brightness grid (so surfaces know how bright it is behind them: `setBackdropField`, no pixels read later),
 *   - the average tone and the most vivid hue (for the accent),
 *   - the wash the sidebars need for legible ink (`washFor`).
 * Cost: two canvas draws and one 24 by 14 read per change of wallpaper, blur or window size (resize is debounced 400 ms); nothing per frame.
 */
import { backdropRange, luminance, scheduleMaterial, setBackdropField, washFor, currentScheme, type Field } from "@calternal/ui";

type Layer = "side" | "card";
const SATURATE: Record<Layer, number> = { side: 120, card: 140 };
const GRID = { cols: 24, rows: 14 };

/** How the wallpaper is applied: blur radii (px) of the two copies, as chosen in settings. */
export interface WallpaperLook {
  sideBlur: number;
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
  const k = Math.min(8, Math.max(1, blur / 8));
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

/** The sidebars' rectangles as fractions of the viewport (the whole viewport on a narrow screen, where one sidebar fills it). */
function sidebarRects(): { x0: number; y0: number; x1: number; y1: number }[] {
  const cs = getComputedStyle(document.documentElement);
  const w = innerWidth || 1;
  if (w < 760) return [{ x0: 0, y0: 0, x1: 1, y1: 1 }];
  const side = parseFloat(cs.getPropertyValue("--side-w")) / w;
  const right = parseFloat(cs.getPropertyValue("--right-w")) / w;
  return [{ x0: 0, y0: 0, x1: side, y1: 1 }, { x0: 1 - right, y0: 0, x1: 1, y1: 1 }];
}

/** Recomputes the wash from the current field and scheme (also called when the scheme changes). */
export function updateWash(sideTintBase = 0.35): void {
  const scheme = currentScheme();
  let min = 1, max = 0;
  for (const r of sidebarRects()) {
    const range = backdropRange(scheme, r);
    min = Math.min(min, range.min);
    max = Math.max(max, range.max);
  }
  root().setProperty("--wash", source ? washFor(scheme, { min, max }, sideTintBase).toFixed(3) : "");
  scheduleMaterial();
}

function clear(): void {
  for (const l of ["side", "card"] as const) {
    root().removeProperty(`--wall-${l}`);
    if (urls[l]) URL.revokeObjectURL(urls[l]!);
    delete urls[l];
  }
  root().removeProperty("--wash");
  setBackdropField(null);
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
    setBackdropField(fieldOf(side));
    updateWash();
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
export function setWallpaper(src: string | null, l?: WallpaperLook): void {
  if (l) look = l;
  if (src === source && !l) return;
  source = src;
  schedule();
}

/** Changes the blur radii (0 shows the picture sharp). */
export function setLook(l: WallpaperLook): void {
  if (l.sideBlur === look.sideBlur && l.cardBlur === look.cardBlur) return;
  look = l;
  if (source) schedule();
}

let resizeTimer = 0;
addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => source && schedule(0), 400); // a bigger window would upscale the small copy further
}, { passive: true });
