/**
 * Adaptive material. A surface that floats over the seamless background responds to how bright the background is
 * behind it: over something that fights its ink it gets a denser tint and a stronger blur; over something calm it keeps the
 * approved see-through look.
 *
 * The brightness is never read from pixels. The background is a 3 by 3 mesh of colours (tokens.json `backdrop`); the
 * same data paints it (scripts/build-tokens.mjs) and answers "how bright is it here?" by interpolating the mesh at a few
 * points inside the surface's rectangle: no canvas, no readback, no per-frame work.
 *
 * Cost: one `getBoundingClientRect` per registered surface per change (resize, scheme change, mount), batched in a single
 * animation frame (all reads, then all writes, so there is no layout thrash), and one 9-point interpolation each. Nothing runs on
 * scroll, because the background is fixed to the viewport and cards do not move over it.
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
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

const MESH: Record<Scheme, number[]> = {
  light: tokens.backdrop.light.map(luminance),
  dark: tokens.backdrop.dark.map(luminance),
};

/** Bilinear brightness of the 3 by 3 mesh at a point (fractions of the viewport). */
function at(mesh: number[], x: number, y: number): number {
  const u = Math.min(Math.max(x, 0), 1) * 2;
  const v = Math.min(Math.max(y, 0), 1) * 2;
  const i = Math.min(Math.floor(u), 1);
  const j = Math.min(Math.floor(v), 1);
  const fu = u - i;
  const fv = v - j;
  const a = mesh[j * 3 + i]!;
  const b = mesh[j * 3 + i + 1]!;
  const c = mesh[(j + 1) * 3 + i]!;
  const d = mesh[(j + 1) * 3 + i + 1]!;
  return a * (1 - fu) * (1 - fv) + b * fu * (1 - fv) + c * (1 - fu) * fv + d * fu * fv;
}

/** Mean brightness of the background inside a rectangle: the mesh sampled at nine points (corners, edges, centre). */
export function behindLuminance(scheme: Scheme, r: Frac, mesh: number[] = MESH[scheme]): number {
  let sum = 0;
  for (const fy of [0, 0.5, 1]) for (const fx of [0, 0.5, 1]) sum += at(mesh, r.x0 + (r.x1 - r.x0) * fx, r.y0 + (r.y1 - r.y0) * fy);
  return sum / 9;
}

/** What a surface is drawn with: tint alpha (0..1) and blur radius (px, before the touch scale). */
export interface Material {
  alpha: number;
  blur: number;
}

/** What kind of surface asks: a small floating control, or one of the big cards. */
export type Role = "glass" | "card";

const num = (s: string): number => parseFloat(s);
const BASE: Record<Scheme, Record<Role, number>> = {
  light: { glass: num(tokens.light["glass-a"]), card: num(tokens.light["card-a"]) },
  dark: { glass: num(tokens.dark["glass-a"]), card: num(tokens.dark["card-a"]) },
};
const BLUR = num(tokens.light["glass-blur"]);

/**
 * The material for a surface whose background has brightness `behind`. How much the background fights the ink (`need`, 0..1):
 * a light scheme has dark ink, so it is the dark parts behind that hurt; a dark scheme is the other way round. The tint grows
 * by up to 0.45 and the blur by up to a fifth as `need` does; with `need` 0 the approved values are returned unchanged.
 * `form` is accepted so the touch tuning can differ later; the touch blur scale itself is a CSS token.
 */
export function material(scheme: Scheme, behind: number, role: Role = "glass", _form: Form = "desktop"): Material {
  const need = scheme === "light" ? Math.max(0, 0.6 - behind) / 0.6 : Math.min(1, Math.max(0, behind - 0.12) / 0.5);
  const base = BASE[scheme][role];
  return { alpha: Math.min(0.92, base + 0.45 * need), blur: Math.round(BLUR * (1 + 0.2 * need)) };
}

// ---- registry: surfaces ask to be adapted; one frame does all the reads, then all the writes ----

type Entry = { role: Role };
const surfaces = new Map<HTMLElement, Entry>();
let frame = 0;
let installed = false;

/** Re-samples every registered surface in the next animation frame (coalesced). */
export function scheduleMaterial(): void {
  if (frame || typeof requestAnimationFrame === "undefined") return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const scheme = currentScheme();
    const form = currentForm();
    const w = innerWidth || 1;
    const h = innerHeight || 1;
    const reads: [HTMLElement, Entry, Frac][] = [];
    for (const [el, e] of surfaces) {
      const r = el.getBoundingClientRect();
      reads.push([el, e, { x0: r.left / w, y0: r.top / h, x1: r.right / w, y1: r.bottom / h }]);
    }
    for (const [el, e, f] of reads) {
      const behind = behindLuminance(scheme, f);
      const m = material(scheme, behind, e.role, form);
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
  scheduleMaterial();
  return {
    update(next: Role) {
      surfaces.set(node, { role: next });
      scheduleMaterial();
    },
    destroy() {
      surfaces.delete(node);
    },
  };
}

/** How many surfaces are registered (tests and the performance script read it). */
export const adaptiveCount = (): number => surfaces.size;
