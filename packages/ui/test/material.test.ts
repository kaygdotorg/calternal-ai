import { describe, expect, it } from "vitest";
import tokens from "../tokens/tokens.json";
import { behindLuminance, luminance, material } from "../src/lib/material";

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
    expect(material("light", 0).alpha).toBeLessThanOrEqual(0.92);
  });
});
