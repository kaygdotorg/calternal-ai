import { describe, expect, it } from "vitest";
import { blurRgba, saturateRgba } from "./blur";

const px = (w: number, h: number, f: (x: number, y: number) => number[]) => { const d = new Uint8ClampedArray(w * h * 4); for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const v = f(x, y); d.set([v[0]!, v[1]!, v[2]!, 255], (y * w + x) * 4); } return d; };

describe("blur for canvases without a filter", () => {
  it("spreads a hard edge into a ramp and keeps the total brightness", () => {
    const w = 64, h = 4, d = px(w, h, (x) => (x < 32 ? [0, 0, 0] : [255, 255, 255]));
    const before = d.reduce((s, v, i) => s + (i % 4 === 3 ? 0 : v), 0);
    blurRgba(d, w, h, 12);
    const row = (x: number) => d[x * 4]!;
    expect(row(10)).toBeLessThan(10); expect(row(54)).toBeGreaterThan(245); // far from the edge: unchanged
    expect(row(30)).toBeGreaterThan(40); expect(row(34)).toBeLessThan(215); // at the edge: a ramp, not a step
    for (let x = 1; x < w; x++) expect(row(x)).toBeGreaterThanOrEqual(row(x - 1)! - 1); // and monotone
    const after = d.reduce((s, v, i) => s + (i % 4 === 3 ? 0 : v), 0);
    expect(Math.abs(after - before) / before).toBeLessThan(0.02);
  });
  it("leaves alpha alone and does nothing for a radius under one pass", () => {
    const d = px(8, 8, (x) => [x * 30, 0, 0]); d[3] = 200;
    const copy = d.slice(); blurRgba(d, 8, 8, 0.5);
    expect([...d]).toEqual([...copy]);
    blurRgba(d, 8, 8, 6); expect(d[3]).toBe(200);
  });
  it("saturation scales the distance from grey", () => {
    const d = px(1, 1, () => [200, 100, 100]); saturateRgba(d, 1.5);
    expect(d[0]).toBeGreaterThan(200); expect(d[1]).toBeLessThan(100);
    const g = px(1, 1, () => [90, 90, 90]); saturateRgba(g, 1.5); expect([g[0], g[1], g[2]]).toEqual([90, 90, 90]);
  });
});
