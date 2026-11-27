/**
 * Generated pictures for the Files and Photos mockups: a deterministic scene (sky, sun, hills, a coast, a skyline, dunes, soft blobs, a laid table) drawn from a seed and a hue, so a thumbnail looks like a photograph's composition without being one.
 * Nothing here is a real photograph and no person is ever drawn. The same spec always draws the same picture, so a thumbnail and the photo it opens agree.
 */
export type SceneKind = "hills" | "coast" | "city" | "dunes" | "blobs" | "table";
export type SceneTime = "day" | "dusk" | "night";
export interface SceneSpec { kind: SceneKind; hue: number; seed: number; time: SceneTime }
export interface Shape { d?: string; x?: number; y?: number; w?: number; h?: number; cx?: number; cy?: number; r?: number; fill: string; opacity?: number; grad?: string }
export interface Scene { sky: [string, string]; shapes: Shape[]; id: string }

/** A small seeded random generator (mulberry32): the same seed gives the same run of numbers. */
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => { a = (a + 0x6d2b79f5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}
const ok = (l: number, c: number, h: number, a = 1) => `oklch(${l.toFixed(3)} ${c.toFixed(3)} ${((h % 360) + 360) % 360} / ${a})`;
/** A ridge line across the 160 x 120 frame: a few smooth waves, closed down to the bottom edge. */
function ridge(r: () => number, base: number, amp: number): string {
  const n = 5, pts: [number, number][] = Array.from({ length: n + 1 }, (_, i) => [(i * 160) / n, base + (r() - 0.5) * 2 * amp]);
  let d = `M0 120 L0 ${pts[0]![1].toFixed(1)}`;
  for (let i = 1; i <= n; i++) { const [x0, y0] = pts[i - 1]!, [x1, y1] = pts[i]!, mx = (x0 + x1) / 2; d += ` C${mx.toFixed(1)} ${y0.toFixed(1)} ${mx.toFixed(1)} ${y1.toFixed(1)} ${x1.toFixed(1)} ${y1.toFixed(1)}`; }
  return `${d} L160 120 Z`;
}
/** Draws the spec as a sky gradient and a list of shapes in back-to-front order. */
export function buildScene(s: SceneSpec): Scene {
  const r = rng(s.seed * 7919 + Math.round(s.hue)), h = s.hue, T = s.time;
  const sky: [string, string] = T === "night" ? [ok(0.26, 0.05, h + 230), ok(0.4, 0.08, h + 200)] : T === "dusk" ? [ok(0.62, 0.11, h + 250), ok(0.88, 0.1, h + 40)] : [ok(0.8, 0.08, h), ok(0.94, 0.04, h + 30)];
  const shapes: Shape[] = [];
  const sun = (y: number) => shapes.push({ cx: 30 + r() * 100, cy: y, r: 6 + r() * 6, fill: T === "night" ? ok(0.95, 0.03, 90) : ok(0.97, 0.06, 80), opacity: T === "night" ? 0.9 : 0.95 });
  const dark = T === "night" ? -0.2 : 0;
  const cloud = () => { if (r() < 0.55 && T !== "night") { const cx = 20 + r() * 120, cy = 14 + r() * 26; for (let k = 0; k < 3; k++) shapes.push({ cx: cx + k * 9 - 9, cy: cy + (k === 1 ? -3 : 0), r: 6 + r() * 5, fill: "oklch(0.99 0 0)", opacity: 0.55 }); } };
  if (s.kind === "hills") {
    sun(14 + r() * 32); cloud();
    const layers = 2 + Math.floor(r() * 3), top = 54 + r() * 24;
    for (let i = 0; i < layers; i++) shapes.push({ d: ridge(r, top + i * ((112 - top) / layers) + r() * 6, 6 + r() * 10), fill: ok(0.78 - i * (0.3 / layers) + dark, 0.06 + i * 0.015, h + 140 * (r() - 0.3) - i * 18) });
  } else if (s.kind === "coast") {
    sun(26 + r() * 22); cloud();
    const hy = 50 + r() * 22;
    shapes.push({ x: 0, y: hy, w: 160, h: 120 - hy, fill: ok(0.62 + dark, 0.09, 200 + (s.seed % 5) * 9) });
    for (let i = 0; i < 7; i++) shapes.push({ x: 10 + r() * 120, y: hy + 6 + i * 8, w: 14 + r() * 26, h: 1.2, fill: ok(0.9, 0.03, 210), opacity: 0.55 });
    shapes.push({ d: ridge(r, 106, 6), fill: ok(0.55 + dark, 0.07, h + 60) });
  } else if (s.kind === "city") {
    if (T === "night") sun(20);
    for (const [layer, l, hmin, hmax] of [[0, 0.5, 34, 66], [1, 0.34, 24, 52]] as const) {
      let x = -4;
      while (x < 160) { const w = 10 + r() * 14, bh = hmin + r() * (hmax - hmin); shapes.push({ x, y: 120 - bh, w, h: bh, fill: ok(l + dark / 2, 0.05, h + 210) });
        if (layer === 1) for (let k = 0; k < 6; k++) shapes.push({ x: x + 2 + r() * (w - 5), y: 120 - bh + 4 + r() * (bh - 10), w: 2, h: 2.4, fill: ok(0.92, 0.1, 85), opacity: T === "day" ? 0.25 : 0.85 });
        x += w + 1 + r() * 2; }
    }
  } else if (s.kind === "dunes") {
    sun(18 + r() * 30); cloud();
    const nd = 2 + Math.floor(r() * 3);
    for (let i = 0; i < nd; i++) shapes.push({ d: ridge(r, 62 + i * (44 / nd) + r() * 8, 8 + r() * 12), fill: ok(0.8 - i * 0.07 + dark, 0.09, h + 30 + i * 12 + r() * 30) });
  } else if (s.kind === "blobs") {
    for (let i = 0; i < 5 + Math.floor(r() * 5); i++) shapes.push({ cx: r() * 160, cy: r() * 120, r: 14 + r() * 40, fill: ok(0.55 + r() * 0.35, 0.12, h + r() * 160), opacity: 0.55, grad: "soft" });
  } else {
    shapes.push({ x: 0, y: 0, w: 160, h: 120, fill: ok(0.78 + dark, 0.05, h + 40) });
    const cx = 50 + r() * 60, cy = 44 + r() * 30, pr = 26 + r() * 20;
    shapes.push({ cx, cy, r: pr, fill: ok(0.96, 0.01, h), opacity: 0.95 }, { cx, cy, r: pr * 0.74, fill: ok(0.9, 0.02, h), opacity: 0.8 });
    for (let i = 0; i < 3 + Math.floor(r() * 6); i++) shapes.push({ cx: cx - pr * 0.4 + r() * pr * 0.8, cy: cy - pr * 0.4 + r() * pr * 0.8, r: 3 + r() * 7, fill: ok(0.5 + r() * 0.25, 0.13, h + 20 + r() * 140) });
    shapes.push({ cx: 24, cy: 100, r: 14, fill: ok(0.62, 0.12, h + 120), opacity: 0.8 }, { cx: 140, cy: 24, r: 11, fill: ok(0.66, 0.1, h + 160), opacity: 0.8 });
  }
  return { sky, shapes, id: `sc${s.seed}-${Math.round(h)}-${s.kind}-${T}` };
}
