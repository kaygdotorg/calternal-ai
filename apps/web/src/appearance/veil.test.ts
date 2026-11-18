import { describe, expect, test } from "vitest";
import { contrastOn, lum, over, ratio, scaled, solve, TARGET, type Ink, type RGB } from "./veil";

const LIGHT: Ink = { ink: [0x26, 0x26, 0x26], ink2: [0x4a, 0x4a, 0x4a], ink3: [0x5f, 0x5f, 0x5f], paper: [0xf7, 0xf7, 0xf7] };
const DARK: Ink = { ink: [0xeb, 0xeb, 0xeb], ink2: [0xc4, 0xc4, 0xc4], ink3: [0xa6, 0xa6, 0xa6], paper: [0x1f, 0x1f, 0x1f] };
const ALL = { blur: true, dim: true, tint: true };
const field = (f: (i: number) => RGB, n = 400): RGB[] => Array.from({ length: n }, (_, i) => f(i));
const dark = field((i) => [10 + (i % 30), 12 + (i % 20), 20 + (i % 25)] as RGB);
const bright = field((i) => [220 + (i % 30), 215 + (i % 35), 200 + (i % 40)] as RGB);

describe("compositing is in encoded sRGB, as CSS does it", () => {
  test("half white over black is 50 % encoded, not 50 % light", () => {
    expect(over([0, 0, 0], [255, 255, 255], 0.5)).toEqual([127.5, 127.5, 127.5]);
    expect(lum(over([0, 0, 0], [255, 255, 255], 0.5))).toBeCloseTo(0.214, 2); // far below 0.5 in linear light
  });
});

describe("the solver reaches its targets on the worst 1 % of the pixels", () => {
  test.each([["light", dark], ["dark", bright]] as const)("%s scheme on a hard wallpaper: primary 7, ink-2 5.5, ink-3 4.5", (scheme, px) => {
    const T = scheme === "light" ? LIGHT : DARK;
    const v = solve(scheme, "side", ALL, px, T);
    expect(v.scheme).toBe(scheme);
    expect(contrastOn(px, T.ink, v).p01).toBeGreaterThanOrEqual(TARGET.ink - 0.05);
    expect(contrastOn(px, v.ink2, v).p01).toBeGreaterThanOrEqual(TARGET.ink2 - 0.05);
    expect(contrastOn(px, v.ink3, v).p01).toBeGreaterThanOrEqual(TARGET.ink3 - 0.05);
  });
  test("an easy wallpaper keeps the default layers and leaves the secondary inks alone", () => {
    const v = solve("light", "side", ALL, bright, LIGHT);
    expect(v.tint.a).toBeCloseTo(0.35, 2);
    expect(v.dim.a).toBeCloseTo(0, 2); // a sidebar's Dim starts at nothing and is raised only as far as the floor needs
    expect(v.ink3).toEqual(LIGHT.ink3);
  });
});

describe("the boxes", () => {
  test("with Dim off the tint is raised, with Tint off the dim is raised, and the text stays readable", () => {
    for (const flags of [{ blur: true, dim: false, tint: true }, { blur: true, dim: true, tint: false }]) {
      const v = solve("light", "side", flags, dark, LIGHT);
      expect(contrastOn(dark, LIGHT.ink, v).p01).toBeGreaterThanOrEqual(TARGET.ink - 0.05);
    }
    expect(solve("light", "side", { blur: true, dim: false, tint: true }, dark, LIGHT).dim.a).toBe(0);
    expect(solve("light", "side", { blur: true, dim: true, tint: false }, dark, LIGHT).tint.a).toBe(0);
  });
  test("with nothing on the surface is opaque paper", () => {
    const v = solve("light", "side", { blur: false, dim: false, tint: false }, dark, LIGHT);
    expect(v.total).toBe(1);
    expect(contrastOn(dark, LIGHT.ink, v).worst).toBeGreaterThan(10);
  });
});

describe("the card", () => {
  test("it starts at the calternal-ui plugin's chat defaults, far denser than a sidebar, and the floor only raises it", () => {
    const mid = field(() => [120, 110, 100] as RGB);
    const card = solve("dark", "card", ALL, mid, DARK), side = solve("dark", "side", ALL, mid, DARK);
    expect(card.tint.a).toBeGreaterThanOrEqual(0.85);
    expect(card.dim.a).toBeGreaterThanOrEqual(0.45);
    expect(card.total).toBeGreaterThan(side.total + 0.25);
    const hard = solve("light", "card", ALL, dark, LIGHT);
    expect(contrastOn(dark, LIGHT.ink, hard).p01).toBeGreaterThanOrEqual(TARGET.ink - 0.05);
  });
  test("the card never flips its ink", () => {
    expect(solve("light", "card", ALL, dark, LIGHT, DARK).scheme).toBe("light");
  });
});

describe("a sidebar may flip its text when that needs clearly less veil", () => {
  test("light scheme over a very dark wallpaper flips to light text", () => {
    const night = field(() => [4, 5, 8] as RGB);
    const v = solve("light", "side", ALL, night, LIGHT, DARK);
    expect(v.scheme).toBe("dark");
    expect(contrastOn(night, DARK.ink, v).p01).toBeGreaterThanOrEqual(TARGET.ink - 0.05);
  });
  test("in the dark scheme nothing flips, however bright the picture (never a light veil in dark mode)", () => {
    const day = field(() => [240, 238, 230] as RGB);
    const v = solve("dark", "side", ALL, day, DARK, LIGHT);
    expect(v.scheme).toBe("dark");
    expect(v.tint.rgb[0]).toBeLessThan(128);
  });
  test("Clear to tinted never goes under a covering veil and keeps the scheme's own paper", () => {
    const v = solve("dark", "side", ALL, field(() => [200, 190, 180] as RGB), DARK);
    const clear = scaled(v, 0.3);
    expect(clear.total).toBeGreaterThanOrEqual(0.3 - 1e-9);
    expect(clear.dim.rgb).toEqual(v.dim.rgb);
    expect(clear.dim.rgb[0]).toBeLessThan(128);
  });
  test("ratio sanity", () => {
    expect(ratio(lum([255, 255, 255]), lum([0, 0, 0]))).toBeCloseTo(21, 3);
  });
});
