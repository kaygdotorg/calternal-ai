import { describe, expect, it } from "vitest";
import { contrast } from "../appearance/accent";
import { hueOf, windowColours } from "./surface";

describe("windowColours", () => {
  for (const scheme of ["light", "dark"] as const) for (const hue of [null, 20, 95, 150, 250, 330]) {
    it(`${scheme} at hue ${hue}: ink 7:1, quiet inks 5.5:1 and 4.5:1`, () => {
      const c = windowColours(scheme, hue);
      expect(contrast(c.ink, c.bg)).toBeGreaterThanOrEqual(7);
      expect(contrast(c.ink2, c.bg)).toBeGreaterThanOrEqual(5.5);
      expect(contrast(c.ink3, c.bg)).toBeGreaterThanOrEqual(4.5);
    });
  }
  it("is pale in light and deep in dark", () => {
    const l = windowColours("light", 40).bg, d = windowColours("dark", 40).bg;
    expect(Math.min(...l)).toBeGreaterThan(215);
    expect(Math.max(...d)).toBeLessThan(70);
  });
});
describe("hueOf", () => {
  it("reads a hue and none for grey", () => {
    expect(Math.abs(hueOf("#0087ff")! - 255)).toBeLessThan(15);
    expect(hueOf("#808080")).toBeNull();
  });
});
