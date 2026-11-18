/**
 * Adaptive material. A surface that floats over the seamless background responds to how bright the background is
 * behind it: over something that fights its ink it gets a denser tint and a stronger blur; over something calm it keeps the
 * approved see-through look.
 *
 * The brightness is never read from pixels. The background is a 3 by 3 mesh of colours (tokens.json `backdrop`); the
 * same data paints it (scripts/build-tokens.mjs) and answers "how bright is it here?" by interpolating the mesh at a few
 * points inside the surface's rectangle: no canvas, no readback, no per-frame work.
 *
 * Cost: one `getBoundingClientRect` per surface that needs it, batched in a single animation frame (all reads, then all writes, so there is no layout
 * thrash), and one 9-point interpolation each. A surface is measured once, when it registers; every registered surface is measured again only when
 * the backdrop or its geometry can have changed (resize, scheme change, a new wallpaper) - mounting one surface (a popover opening on a key) never
 * re-reads the others. Nothing runs on scroll, because the background is fixed to the viewport and cards do not move over it.
 */
import tokens from "../../tokens/tokens.json" with { type: "json" };
import { currentForm, currentScheme, onScheme, type Form, type Scheme } from "./theme";

/** Relative luminance (0 black .. 1 white) of an `#rrggbb` colour, per WCAG. */
export function luminance(hex: string): number {
  const lin = (i: number) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(1) + 0.7152 * lin(3) + 0.0722 * lin(5);
}

/** A rectangle as fractions of the viewport (0..1). */
export interface Frac {
  /** Left edge as a fraction of the viewport's width. */
  x0: number;
  /** Top edge as a fraction of the viewport's height. */
  y0: number;
  /** Right edge as a fraction of the viewport's width. */
  x1: number;
  /** Bottom edge as a fraction of the viewport's height. */
  y1: number;
}

/**
 * What the surfaces are drawn over, as a grid of brightness values (0..1, relative luminance) laid over the viewport: `cols` by `rows`,
 * row-major, cell centres at (i + 0.5) / cols. The default is the background's own 3 by 3 colour mesh; a wallpaper replaces it with a
 * grid read once from its blurred copy (`setBackdropField`). Nothing here ever reads pixels at runtime.
 */
export interface Field {
  /** Columns of the brightness grid laid over the viewport. */
  cols: number;
  /** Rows of the brightness grid. */
  rows: number;
  /** Relative luminance (0 dark to 1 light) of each cell, row-major; read, never changed. */
  lum: ArrayLike<number>;
}

const MESH: Record<Scheme, Field> = {
  light: { cols: 3, rows: 3, lum: tokens.backdrop.light.map(luminance) },
  dark: { cols: 3, rows: 3, lum: tokens.backdrop.dark.map(luminance) },
};
let wallpaper: Field | null = null;

/** Uses `f` (a wallpaper's brightness grid) as what surfaces sit on, or the background mesh again when null. Re-samples every surface. */
export function setBackdropField(f: Field | null): void {
  wallpaper = f;
  scheduleMaterial();
}

/** Bilinear brightness of a grid at a point (fractions of the viewport); the grid's outer cells extend to the edges. */
function at(f: Field, x: number, y: number): number {
  const u = Math.min(Math.max(x * f.cols - 0.5, 0), f.cols - 1);
  const v = Math.min(Math.max(y * f.rows - 0.5, 0), f.rows - 1);
  const i = Math.min(Math.floor(u), f.cols - 2 < 0 ? 0 : f.cols - 2);
  const j = Math.min(Math.floor(v), f.rows - 2 < 0 ? 0 : f.rows - 2);
  const fu = u - i;
  const fv = v - j;
  const g = (a: number, b: number) => f.lum[Math.min(b, f.rows - 1) * f.cols + Math.min(a, f.cols - 1)]!;
  return g(i, j) * (1 - fu) * (1 - fv) + g(i + 1, j) * fu * (1 - fv) + g(i, j + 1) * (1 - fu) * fv + g(i + 1, j + 1) * fu * fv;
}

const asField = (m: Field | number[]): Field => (Array.isArray(m) ? { cols: 3, rows: 3, lum: m } : m);

/** Mean brightness of the backdrop inside a rectangle: the grid sampled at nine points (corners, edges, centre). */
export function behindLuminance(scheme: Scheme, r: Frac, mesh?: Field | number[]): number {
  const f = mesh ? asField(mesh) : (wallpaper ?? MESH[scheme]);
  let sum = 0;
  for (const fy of [0, 0.5, 1]) for (const fx of [0, 0.5, 1]) sum += at(f, r.x0 + (r.x1 - r.x0) * fx, r.y0 + (r.y1 - r.y0) * fy);
  return sum / 9;
}

/** The darkest and brightest cell of the current backdrop inside a rectangle: what the worst legible-text case is. */
export function backdropRange(scheme: Scheme, r: Frac): { min: number; max: number } {
  const f = wallpaper ?? MESH[scheme];
  let min = 1;
  let max = 0;
  for (let j = 0; j < f.rows; j++) {
    const y = (j + 0.5) / f.rows;
    if (y < r.y0 || y > r.y1) continue;
    for (let i = 0; i < f.cols; i++) {
      const x = (i + 0.5) / f.cols;
      if (x < r.x0 || x > r.x1) continue;
      const v = f.lum[j * f.cols + i]!;
      if (v < min) min = v;
      if (v > max) max = v;
    }
  }
  return max < min ? { min: 0.5, max: 0.5 } : { min, max };
}

// CSS composites in gamma-encoded sRGB, not in linear light: a veil of alpha a over a ground g is (1 - a) * enc(g) + a * enc(veil), per channel.
const toEnc = (l: number): number => (l <= 0.0031308 ? 12.92 * l : 1.055 * l ** (1 / 2.4) - 0.055);

/**
 * The wash (0..1 alpha of the scheme's paper colour) a flat surface on the backdrop needs so its secondary ink (`ink-3`) reaches 4.5:1 over the
 * worst cell in `range`: a light veil in the light scheme (dark ink needs a light ground), a dark one in the dark scheme. Never below `base`, never above
 * 0.9. The mixing is done on encoded values, the way the browser composites it (an earlier version mixed in linear light and delivered 3.0 to 3.8:1 where it
 * promised 4.5:1). Grey cells are assumed; `appearance/veil.ts` solves the shell's real surfaces from their pixels.
 */
export function washFor(scheme: Scheme, range: { min: number; max: number }, base: number): number {
  const ink = luminance(tokens[scheme]["ink-3"]);
  const veil = toEnc(luminance(tokens[scheme].paper));
  if (scheme === "light") {
    const need = toEnc(4.5 * (ink + 0.05) - 0.05); // the lightest ground ink-3 may sit on, encoded
    const w = toEnc(range.min);
    return Math.min(0.9, Math.max(base, need <= w ? 0 : (need - w) / (veil - w)));
  }
  const allow = toEnc((ink + 0.05) / 4.5 - 0.05); // the darkest ground ceiling, encoded
  const w = toEnc(range.max);
  return Math.min(0.9, Math.max(base, w <= allow ? 0 : (w - allow) / (w - veil)));
}

/** What a surface is drawn with: tint alpha (0..1) and blur radius (px, before the touch scale). */
export interface Material {
  /** Tint opacity of the glass, 0 to 1. */
  alpha: number;
  /** Blur radius of the glass in px, before the touch scale. */
  blur: number;
}

/** What kind of surface asks: a small floating control, or one of the big cards. */
export type Role = "glass" | "card";

const num = (s: string): number => parseFloat(s);
/** The most fill a glass pill gains where its backdrop fights its glyphs (the plugin's `--cf-pill-wash-max`). */
const WASH_MAX = 0.35;
const BASE: Record<Scheme, Record<Role, number>> = {
  light: { glass: num(tokens.light["glass-a"]), card: num(tokens.light["glass-a"]) },
  dark: { glass: num(tokens.dark["glass-a"]), card: num(tokens.dark["glass-a"]) },
};
const BLUR = num(tokens.light["glass-blur"]);

/**
 * The material for a surface whose background has brightness `behind`. How much the background fights the ink (`need`, 0..1):
 * a light scheme has dark ink, so it is the dark parts behind that hurt; a dark scheme is the other way round. The tint grows
 * by up to the wash the plugin allows (0.35) and the blur by up to a fifth as `need` does; with `need` 0 the approved values are returned unchanged.
 * `form` is accepted so the touch tuning can differ later; the touch blur scale itself is a CSS token.
 */
export function material(scheme: Scheme, behind: number, role: Role = "glass", _form: Form = "desktop"): Material {
  const need = scheme === "light" ? Math.max(0, 0.6 - behind) / 0.6 : Math.min(1, Math.max(0, behind - 0.12) / 0.5);
  const base = BASE[scheme][role];
  return { alpha: Math.min(0.4, base + WASH_MAX * need), blur: Math.round(BLUR * (1 + 0.2 * need)) };
}

/**
 * Where a floating surface's tint comes from when the app can read what is really behind it: `rect` is the surface in CSS pixels; the answer is the tint opacity (0..1) its glass needs so ink reaches the shell's
 * readability floor over the real backdrop (the wallpaper copy under the sidebars' veil, or the card's own surface), or null for "no reading: use the brightness field". Registered once by the app that owns the pictures
 * (`appearance/wallpaper.ts`); every menu, popover, picker, sheet, dialog, tooltip, toast and pill reads it through `adaptive`, so there is one rule, the one the sidebars follow.
 */
export type MaterialSource = (rect: { left: number; top: number; right: number; bottom: number }, scheme: Scheme) => number | null;
let source: MaterialSource | null = null;
/** Registers (or, with null, removes) the source of floating surfaces' readability tint, and re-reads every surface. */
export function setMaterialSource(fn: MaterialSource | null): void {
  source = fn;
  scheduleMaterial();
}

// ---- registry: surfaces ask to be adapted; one frame does all the reads, then all the writes ----

type Entry = { role: Role };
const surfaces = new Map<HTMLElement, Entry>();
let frame = 0;
let installed = false;
/** Surfaces waiting to be measured for the first time (or after a role change); `all` when the backdrop or the geometry changed and every surface is. */
const fresh = new Set<HTMLElement>();
let all = false;

/** Re-samples every registered surface in the next animation frame (coalesced): the backdrop changed (a wallpaper, the scheme) or the window did. */
export function scheduleMaterial(): void {
  all = true;
  flushLater();
}

function flushLater(): void {
  if (frame || typeof requestAnimationFrame === "undefined") return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const todo = all ? surfaces.keys() : fresh.values();
    all = false;
    const scheme = currentScheme();
    const form = currentForm();
    const w = innerWidth || 1;
    const h = innerHeight || 1;
    const reads: [HTMLElement, Entry, Frac, { left: number; top: number; right: number; bottom: number }][] = [];
    for (const el of todo) {
      const e = surfaces.get(el);
      if (!e) continue;
      const r = el.getBoundingClientRect();
      reads.push([el, e, { x0: r.left / w, y0: r.top / h, x1: r.right / w, y1: r.bottom / h }, { left: r.left, top: r.top, right: r.right, bottom: r.bottom }]);
    }
    fresh.clear();
    for (const [el, e, f, rect] of reads) {
      const behind = behindLuminance(scheme, f);
      const m = material(scheme, behind, e.role, form);
      const read = source?.(rect, scheme); // (the source reads pictures it already holds, not the page)
      if (read != null) m.alpha = Math.min(0.9, Math.max(BASE[scheme][e.role], read)); // the real backdrop decides when the app can read it; the field only guesses
      el.style.setProperty("--mat-a", m.alpha.toFixed(3));
      el.style.setProperty("--mat-blur", `${m.blur}px`);
      el.dataset.behind = behind > 0.5 ? "light" : "dark";
    }
  });
}

function install(): void {
  if (installed) return;
  installed = true;
  addEventListener("resize", scheduleMaterial, { passive: true });
  onScheme(scheduleMaterial);
}

/** Svelte action: adapts the surface's material to the background behind it. `role` is "glass" (default) or "card". */
export function adaptive(node: HTMLElement, role: Role = "glass") {
  install();
  surfaces.set(node, { role });
  fresh.add(node);
  flushLater();
  return {
    update(next: Role) {
      surfaces.set(node, { role: next });
      fresh.add(node);
      flushLater();
    },
    destroy() {
      surfaces.delete(node);
      fresh.delete(node);
    },
  };
}

