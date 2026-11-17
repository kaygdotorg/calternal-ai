import { describe, expect, it } from "vitest";
import tokens from "../tokens/tokens.json";
import { backdropRange, behindLuminance, luminance, material, setBackdropField, washFor } from "../src/lib/material";

describe("luminance", () => {
  it("orders black, grey and white", () => {
    expect(luminance("#000000")).toBe(0);
    expect(luminance("#ffffff")).toBeCloseTo(1, 5);
    expect(luminance("#808080")).toBeGreaterThan(0.2);
    expect(luminance("#808080")).toBeLessThan(0.3);
  });
});

describe("behindLuminance", () => {
  const whole = { x0: 0, y0: 0, x1: 1, y1: 1 };
  it("is bright for the light mesh and dark for the dark mesh", () => {
    expect(behindLuminance("light", whole)).toBeGreaterThan(0.6);
    expect(behindLuminance("dark", whole)).toBeLessThan(0.05);
  });
  it("follows a uniform mesh exactly, wherever the rectangle is", () => {
    const flat = Array(9).fill(0.4);
    expect(behindLuminance("light", { x0: 0.2, y0: 0.3, x1: 0.6, y1: 0.9 }, flat)).toBeCloseTo(0.4, 10);
  });
  it("sees a bright corner only where the rectangle reaches it", () => {
    const mesh = [1, 0, 0, 0, 0, 0, 0, 0, 0];
    expect(behindLuminance("light", { x0: 0, y0: 0, x1: 0.1, y1: 0.1 }, mesh)).toBeGreaterThan(behindLuminance("light", { x0: 0.6, y0: 0.6, x1: 1, y1: 1 }, mesh));
  });
});

describe("material", () => {
  it("keeps the approved values over a calm backdrop", () => {
    const m = material("light", 0.8);
    expect(m.alpha).toBeCloseTo(parseFloat(tokens.light["glass-a"]), 5);
    expect(m.blur).toBe(parseInt(tokens.light["glass-blur"]));
  });
  it("gets denser and blurrier where the backdrop fights the ink", () => {
    const calm = material("light", 0.8);
    const busy = material("light", 0.1);
    expect(busy.alpha).toBeGreaterThan(calm.alpha);
    expect(busy.blur).toBeGreaterThan(calm.blur);
    expect(material("dark", 0.9).alpha).toBeGreaterThan(material("dark", 0.02).alpha);
  });
  it("never exceeds an alpha that would hide the backdrop entirely", () => {
    expect(material("light", 0).alpha).toBeLessThanOrEqual(0.4);
  });
});

describe("wallpaper field", () => {
  const dark = { cols: 4, rows: 2, lum: [0.02, 0.02, 0.9, 0.9, 0.02, 0.02, 0.9, 0.9] };
  it("replaces the mesh and reports the range inside a region", () => {
    setBackdropField(dark);
    try {
      expect(behindLuminance("light", { x0: 0, y0: 0, x1: 0.2, y1: 1 })).toBeLessThan(0.1);
      expect(behindLuminance("light", { x0: 0.8, y0: 0, x1: 1, y1: 1 })).toBeGreaterThan(0.8);
      expect(backdropRange("light", { x0: 0, y0: 0, x1: 0.5, y1: 1 })).toEqual({ min: 0.02, max: 0.02 });
    } finally {
      setBackdropField(null);
    }
  });
  it("washes a dark wallpaper white for dark ink, and a bright one black for light ink", () => {
    expect(washFor("light", { min: 0.02, max: 0.1 }, 0.35)).toBeGreaterThan(0.55);
    expect(washFor("light", { min: 0.85, max: 0.9 }, 0.35)).toBe(0.35);
    expect(washFor("dark", { min: 0.7, max: 0.9 }, 0.35)).toBeGreaterThan(0.8);
    expect(washFor("dark", { min: 0.0, max: 0.02 }, 0.35)).toBe(0.35);
    expect(washFor("light", { min: 0, max: 0 }, 0.35)).toBeLessThanOrEqual(0.9);
  });
});
