import { describe, expect, it } from "vitest";
import { contrast } from "./accent";
import { PRESETS } from "./presets";
import { RECIPES, darken, sample } from "./recipe";
import { guard, mixLab, tokensFor } from "./theme";

const hex = (s: string) => [1, 3, 5].map((i) => parseInt(s.slice(i, i + 2), 16)) as [number, number, number];

describe("presets", () => {
  it("are the twelve of the first set, with unique ids, and community palettes keep their licence and source", () => {
    expect(PRESETS).toHaveLength(12);
    expect(new Set(PRESETS.map((p) => p.id)).size).toBe(12);
    for (const p of PRESETS.filter((x) => x.theme)) {
      expect(p.theme!.licence, p.id).toBeTruthy();
      expect(p.theme!.source, p.id).toMatch(/^https?:\/\/|repository/);
    }
  });
  it("every palette in both schemes derives tokens whose secondary ink and text uses reach 4.5:1 on its own paper", () => {
    for (const p of PRESETS.filter((x) => x.theme)) for (const s of ["light", "dark"] as const) {
      const pal = p.theme![s];
      const t = tokensFor(pal, s);
      const paper = hex(pal.paper);
      for (const k of ["ink-3", "accent", "danger", "good", "warn", "info"]) expect(contrast(hex(t[k]!), paper), `${p.id} ${s} ${k}`).toBeGreaterThanOrEqual(4.5);
    }
  });
  it("the guard moves a colour only as far as it must", () => {
    const paper = hex("#ffffff");
    expect(guard(hex("#000000"), paper, [0, 0, 0])).toEqual([0, 0, 0]);
    const g = guard(hex("#999999"), paper, [0, 0, 0]);
    expect(contrast(g, paper)).toBeGreaterThanOrEqual(4.5);
    expect(g[0]).toBeGreaterThan(80);
  });
  it("mixes in OKLab: the midpoint of two greys is a grey of mid lightness", () => {
    const m = mixLab([0, 0, 0], [255, 255, 255], 0.5);
    expect(Math.abs(m[0] - m[1])).toBeLessThan(2);
    expect(m[0]).toBeGreaterThan(90);
    expect(m[0]).toBeLessThan(140);
  });
});

describe("recipes", () => {
  it("are deterministic, calm (low chroma) and have a darker twin with the same order of lightness", () => {
    for (const r of RECIPES) {
      expect(sample(r, "light", 0.3, 0.4)).toEqual(sample(r, "light", 0.3, 0.4));
      expect(r.points.every((p) => p.C <= 0.065), r.id).toBe(true);
      const l = sample(r, "light", 0.5, 0.5), d = sample(r, "dark", 0.5, 0.5);
      expect(d[0] + d[1] + d[2]).toBeLessThan(l[0] + l[1] + l[2]);
    }
    expect(darken(0.9, 0.05)[0]).toBeGreaterThan(darken(0.5, 0.05)[0]);
  });
});
