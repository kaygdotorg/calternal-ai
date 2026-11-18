import { describe, expect, it } from "vitest";
import { contrast } from "../appearance/accent";
import { FLOOR, frostDensity, MARGIN } from "./frost";

const paper: [number, number, number] = [247, 247, 247], ink: [number, number, number] = [38, 38, 38], ink3: [number, number, number] = [107, 107, 107];
describe("frostDensity (the old Calternal's frostRange)", () => {
  it("is the margin alone when the words already read on the worst colour", () => {
    expect(frostDensity(paper, ink, ink3, [200, 200, 200], [255, 255, 255])).toBeCloseTo(MARGIN);
  });
  it("rises as the backdrop fights the ink, and the result reads at its worst colour", () => {
    const lo: [number, number, number] = [10, 10, 10], hi: [number, number, number] = [255, 255, 255];
    const d = frostDensity(paper, ink, ink3, lo, hi);
    expect(d).toBeGreaterThan(0.3);
    const mix = (b: [number, number, number]) => [0, 1, 2].map((i) => paper[i]! * d + b[i]! * (1 - d)) as [number, number, number];
    expect(contrast(ink, mix(lo))).toBeGreaterThanOrEqual(FLOOR.body);
    expect(contrast(ink3, mix(hi))).toBeGreaterThanOrEqual(FLOOR.secondary);
  });
  it("is solid when even solid paper cannot read", () => {
    expect(frostDensity([120, 120, 120], [128, 128, 128], [130, 130, 130], [0, 0, 0], [255, 255, 255])).toBe(1);
  });
});
