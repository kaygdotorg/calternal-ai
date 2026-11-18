import { describe, expect, it } from "vitest";
import { centring, dragBoundary, GAP, heightsOf, leftOf, maxScroll, nearColumns, nextPreset, proportionFor, proportionOf, scrollTo, stripWidth, widthOf, widthsOf, type Col, type Win } from "./layout";

const w = (id: string, weight = 1): Win => ({ id, kind: "threads", item: id, weight, frosted: false, locked: false });
const col = (id: string, p: number | null = null): Col => ({ id, p, wins: [w(id)] });

describe("niri's proportions", () => {
  it("1/n columns and their gaps fill the canvas exactly", () => {
    for (const n of [1, 2, 3, 4]) {
      const ws = Array.from({ length: n }, () => widthOf(1 / n, 1000));
      expect(stripWidth(ws)).toBeCloseTo(1000);
    }
  });
  it("one window fills the canvas minus a gap all round (about 99 % of a wide canvas) and is centred", () => {
    const ws = widthsOf([col("a")], 1000);
    expect(ws[0]).toBe(1000 - 2 * GAP);
    expect(centring(ws, 1000)).toBe(0);
    expect(centring([widthOf(2 / 3, 1000)], 1000)).toBeCloseTo((1000 - stripWidth([widthOf(2 / 3, 1000)])) / 2);
  });
  it("two share it, half each; three keep half each and the strip scrolls", () => {
    expect(proportionOf(col("a"), 2, 1000)).toBe(0.5);
    const three = widthsOf([col("a"), col("b"), col("c")], 1000);
    expect(three.every((x) => Math.abs(x - widthOf(0.5, 1000)) < 0.01)).toBe(true);
    expect(maxScroll(three, 1000)).toBeGreaterThan(0);
  });
  it("a sized column keeps its proportion; a phone is always the full width", () => {
    expect(proportionOf(col("a", 1 / 3), 3, 1000)).toBeCloseTo(1 / 3);
    expect(proportionOf(col("a", 1 / 3), 3, 390)).toBe(1);
  });
});

describe("placement", () => {
  const ws = [500, 400, 600, 300, 500];
  it("places columns by arithmetic", () => {
    expect(leftOf(ws, 0)).toBe(GAP);
    expect(leftOf(ws, 2)).toBe(GAP + 500 + GAP + 400 + GAP);
  });
  it("mounts only the columns near the viewport", () => {
    expect(nearColumns(ws, 0, 800, 0)).toEqual([0, 1]);
    expect(nearColumns(ws, 0, 800, 300)).toEqual([0, 1, 2]);
  });
  it("scrolls the least to show a column, clamped; a wide column is centred", () => {
    expect(scrollTo(ws, 0, 0, 800)).toBe(0);
    expect(scrollTo(ws, 2, 0, 800)).toBe(leftOf(ws, 2) + 600 + GAP - 800);
    expect(scrollTo(ws, 1, 9000, 800)).toBeLessThanOrEqual(maxScroll(ws, 800));
    expect(scrollTo([780], 0, 0, 790)).toBeCloseTo(3);
  });
});

describe("presets and edge drags", () => {
  it("cycles 1/3, 1/2, 2/3, full, and from a custom size goes to the next preset above", () => {
    expect(nextPreset(1 / 3)).toBeCloseTo(0.5);
    expect(nextPreset(2 / 3)).toBe(1);
    expect(nextPreset(1)).toBeCloseTo(1 / 3);
    expect(nextPreset(0.4)).toBeCloseTo(0.5);
    expect(nextPreset(1 / 3, true)).toBe(1);
  });
  it("snaps a dragged width to a preset within 12 px and clamps", () => {
    expect(proportionFor(widthOf(0.5, 1000) + 7, 1000)).toBe(0.5);
    expect(proportionFor(widthOf(0.5, 1000) + 40, 1000)).not.toBe(0.5);
    expect(proportionFor(5000, 1000)).toBe(1);
    expect(proportionFor(10, 1000)).toBeCloseTo(0.2);
  });
});

describe("stacked windows", () => {
  it("share a column's height by weight", () => {
    const h = heightsOf([w("a", 1), w("b", 3)], 412);
    expect(h[0]! + h[1]! + GAP).toBeCloseTo(412);
    expect(h[1]! / h[0]!).toBeCloseTo(3);
  });
  it("drag a boundary between two neighbours within limits", () => {
    const wins = [w("a"), w("b")];
    const n = dragBoundary(wins, 0, 50, 412);
    expect(n[0]).toBeGreaterThan(1); expect(n[1]).toBeLessThan(1);
    expect(dragBoundary(wins, 0, 5000, 412)).toEqual([1, 1]);
  });
});
