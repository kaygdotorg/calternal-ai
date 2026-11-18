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
import { cssPx, luminance, scheduleMaterial, setBackdropField, setMaterialSource, currentScheme, type Field } from "@calternal/ui";
import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
import { surfaces } from "./surfaces.svelte";
import { blurRgba, saturateRgba } from "./blur";
import { setThemeColor } from "../shell/chrome";
import { over, p01, lum, ratio, TARGET, scaled, solve, type Flags, type Ink, type RGB, type Role, type Scheme, type Veil } from "./veil";

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

/**
 * The darkest and the lightest colour the wallpaper shows (the 1st and 99th percentile by luminance of its pixels, sampled from the small copy) and its mean, or null with no wallpaper. The canvas's frosted windows are as dense as the least that
 * still reads against these two (canvas/frost.ts, ported from the old Calternal's `frostRange`); its opaque windows keep their lightness away from the mean. Cost: a sample of a 160 px copy, a sort, once per call.
 */
export function wallpaperExtremes(): { lo: RGB; hi: RGB; mean: RGB } | null {
  if (!copies || !source) return null;
  const px = sample(copies.card, { x0: 0, y0: 0, x1: 1, y1: 1 });
  if (!px.length) return null;
  const by = [...px].sort((a, b) => lum(a) - lum(b));
  const at = (q: number) => by[Math.min(by.length - 1, Math.max(0, Math.floor(q * (by.length - 1))))]!;
  const mean = [0, 1, 2].map((i) => px.reduce((t, p) => t + p[i]!, 0) / px.length) as RGB;
  return { lo: at(0.01), hi: at(0.99), mean };
}

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

/** Whether this browser's canvas honours `filter` (Chrome and Firefox do; Safari ignores it, so the blur is done by hand there). */
const canvasFilter = ((): boolean => { if ((globalThis as { __noCanvasFilter?: boolean }).__noCanvasFilter) return false; /* a test turns it off to exercise the by-hand blur anywhere */ const c = document.createElement("canvas").getContext("2d"); if (!c || !("filter" in c)) return false; c.filter = "blur(2px)"; return c.filter === "blur(2px)"; })();
/** Device pixels a pixel of the copy covers at most: the copy is a blur, so it can be coarse, but a browser that scales it up without smoothing (Safari did) must not show blocks, and at 4 device pixels they are too small to see. */
const COARSE = 4;

/**
 * Renders the picture cover-fitted to the viewport, blurred and saturated, into a canvas sized to the screen's own pixels (one copy pixel per 4 device pixels: a 390 wide phone at 3x gets 292 px across, a 1440 wide window at 1x
 * gets 360), so nothing is scaled up much and no block shows at 1x, 2x or 3x. The blur is a canvas filter where there is one, else the by-hand blur in `blur.ts`. Cost: one pass at start and when the window or the picture changes;
 * the blob is held while the wallpaper is, about 15 to 60 KB of JPEG a copy.
 */
function render(img: HTMLImageElement, blur: number, saturate: number): HTMLCanvasElement {
  const dpr = Math.max(1, devicePixelRatio || 1);
  const per = COARSE / dpr; // CSS px per copy pixel
  const w = Math.max(16, Math.round(innerWidth / per));
  const h = Math.max(16, Math.round(innerHeight / per));
  const r = blur / per; // the blur radius in copy pixels
  const m = Math.ceil(2 * r); // draw past the edges so the blur never fades them to transparent
  const scale = Math.max((w + 2 * m) / img.naturalWidth, (h + 2 * m) / img.naturalHeight);
  const dw = img.naturalWidth * scale;
  const dh = img.naturalHeight * scale;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: !canvasFilter })!;
  if (canvasFilter) {
    ctx.filter = `blur(${r}px) saturate(${saturate}%)`;
    ctx.drawImage(img, (w - dw) / 2, (h - dh) / 2, dw, dh);
    return canvas;
  }
  // No canvas filter: draw the picture larger than the copy by the margin, take it down in halves (each one a smooth average, like mip-mapping, so no aliasing), then blur by hand.
  ctx.imageSmoothingQuality = "high";
  let cur: CanvasImageSource = img, cw = img.naturalWidth, ch = img.naturalHeight;
  while (cw / 2 >= dw && ch / 2 >= dh) {
    const half = document.createElement("canvas");
    half.width = Math.round(cw / 2); half.height = Math.round(ch / 2);
    const hc = half.getContext("2d")!;
    hc.imageSmoothingQuality = "high";
    hc.drawImage(cur, 0, 0, half.width, half.height);
    cur = half; cw = half.width; ch = half.height;
  }
  ctx.drawImage(cur, (w - dw) / 2, (h - dh) / 2, dw, dh);
  const px = ctx.getImageData(0, 0, w, h);
  blurRgba(px.data, w, h, r);
  saturateRgba(px.data, saturate / 100);
  ctx.putImageData(px, 0, 0);
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
  // ONE veil for everything that is not the card: the wallpaper and a single veil span the whole window and both sidebars are drawn on them (docs/ui-shell.md "The background"), so the veil is solved once, from the
  // pixels under both sidebars together (the ink must read on the worse of the two), and written under both names (`--l-*` and `--r-*` stay, equal, for what reads them). The card has its own, on top.
  const jobs: [string[], Role, RGB[]][] = [[["l", "r"], "side", [...px(copies?.side, r.left), ...px(copies?.side, r.right)]], [["c"], "card", px(copies?.card, r.card)]];
  const out: Record<string, Veil> = {};
  for (const [keys, role, pixels] of jobs) for (const k of keys) {
    const v: Veil = craftLight && scheme === "light" ? { scheme: "light", tint: { a: 0.92, rgb: [255, 255, 255] }, dim: { a: 0, rgb: [0, 0, 0] }, ink2: T.ink2, ink3: T.ink3, total: 0.92, ratio: 99 } : solve(scheme, role, flags, pixels, T, other);
    const w: Veil = craftLight && scheme === "light" ? v : scaled(v, veilScale);
    out[k] = w;
    const ink2 = w.ink2, ink3 = w.ink3;
    root().setProperty(`--${k}-tint`, rgbCss(w.tint.rgb));
    root().setProperty(`--${k}-tint-a`, w.tint.a.toFixed(3));
    root().setProperty(`--${k}-dim`, rgbCss(w.dim.rgb));
    root().setProperty(`--${k}-dim-a`, w.dim.a.toFixed(3));
    root().setProperty(`--${k}-ink2`, `rgb(${rgbCss(ink2)})`);
    root().setProperty(`--${k}-ink3`, `rgb(${rgbCss(ink3)})`);
    root().setProperty(`--veil-${k}`, `linear-gradient(rgb(var(--${k}-dim) / var(--${k}-dim-a)) 0 0), linear-gradient(rgb(var(--${k}-tint) / var(--${k}-tint-a)) 0 0)`);
  }
  // The browser's own bars (Safari's toolbar and status bar) take the colour at the top of the page: the card's top strip under its veil (shell/chrome.ts).
  const edge = out.c;
  if (edge) {
    const top = px(copies?.card, { x0: 0, y0: 0, x1: 1, y1: 0.04 });
    const m = top.reduce((a, p) => [a[0] + p[0], a[1] + p[1], a[2] + p[2]], [0, 0, 0]).map((v) => v / Math.max(1, top.length)) as RGB;
    const c = over(over(m, edge.tint.rgb, edge.tint.a), edge.dim.rgb, edge.dim.a);
    setThemeColor(`rgb(${Math.round(c[0])} ${Math.round(c[1])} ${Math.round(c[2])})`);
  }
  const flip = (v: Veil | undefined) => (v && v.scheme !== scheme ? v.scheme : null);
  surfaces.left = flip(out.l);
  surfaces.right = flip(out.r);
  // Floating surfaces (menus, popovers, pickers, sheets, dialogs, tooltips, toasts, pills) read what is really behind them from the same pictures and veils and are tinted just enough for their ink to reach the same
  // readability floor as the sidebars (packages/ui lib/material.ts, docs/ui-design-system.md "One material for floating surfaces").
  const vl = out.l, vc = out.c, cardFrac = r.card;
  setMaterialSource((rect, sch) => {
    if (sch !== scheme || !copies || !vl || !vc) return null;
    const w = innerWidth || 1, h = innerHeight || 1;
    const clamp = (v: number) => Math.min(1, Math.max(0, v));
    const f: Frac = { x0: clamp(rect.left / w), y0: clamp(rect.top / h), x1: clamp(rect.right / w), y1: clamp(rect.bottom / h) };
    if (f.x1 - f.x0 < 0.002 || f.y1 - f.y0 < 0.002) return null;
    const onCard = (f.x0 + f.x1) / 2 > cardFrac.x0 && (f.x0 + f.x1) / 2 < cardFrac.x1 && (f.y0 + f.y1) / 2 > cardFrac.y0 && (f.y0 + f.y1) / 2 < cardFrac.y1;
    const veil = onCard ? vc : vl;
    const behind = sample(onCard ? copies.card : copies.side, f).map((p) => over(over(p, veil.tint.rgb, veil.tint.a), veil.dim.rgb, veil.dim.a));
    const ok = (a: number) => p01(behind.map((g) => ratio(lum(over(g, T.paper, a)), lum(T.ink)))) >= TARGET.ink + 0.35;
    if (ok(0)) return 0;
    let lo = 0, hi = 0.9;
    if (!ok(hi)) return hi;
    for (let i = 0; i < 14; i++) { const m = (lo + hi) / 2; if (ok(m)) hi = m; else lo = m; }
    return hi;
  });
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
  // The picture itself, unblurred, for the canvas experiment's sharp area around its windows (canvas/Canvas.svelte); the URL is already the one being decoded, so this costs nothing.
  if (src) root().setProperty("--wall-sharp", `url("${src}")`); else root().removeProperty("--wall-sharp");
  schedule();
}

let resizeTimer = 0;
addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(() => source && schedule(0), 400); // a bigger window would upscale the small copy further
}, { passive: true });
