import { describe, expect, it } from "vitest";
import { colLeft, dragBoundary, GAP, heightsOf, maxScroll, nearColumns, scrollTo, stripWidth, type Col, type Win } from "./layout";

const w = (id: string, weight = 1): Win => ({ id, kind: "threads", item: id, weight, frosted: false, locked: false });
const col = (id: string, width: number, wins: Win[] = [w(id)]): Col => ({ id, w: width, wins });
const cols = [col("a", 500), col("b", 400), col("c", 600), col("d", 300), col("e", 500)];

describe("layout", () => {
  it("places columns by arithmetic", () => {
    expect(colLeft(cols, 0)).toBe(GAP);
    expect(colLeft(cols, 2)).toBe(GAP + 500 + GAP + 400 + GAP);
    expect(stripWidth(cols)).toBe(GAP + cols.reduce((s, c) => s + c.w + GAP, 0));
  });
  it("mounts only the columns near the viewport", () => {
    expect(nearColumns(cols, 0, 800, 0)).toEqual([0, 1]);
    expect(nearColumns(cols, 0, 800, 200)).toEqual([0, 1, 2]);
    expect(nearColumns(cols, 2000, 800, 0)).toEqual([3, 4].filter((i) => colLeft(cols, i) <= 2800 && colLeft(cols, i) + cols[i]!.w >= 2000));
  });
  it("scrolls the least to show a column, clamped", () => {
    expect(scrollTo(cols, 0, 0, 800)).toBe(0);
    expect(scrollTo(cols, 2, 0, 800)).toBe(colLeft(cols, 2) + 600 + GAP - 800);
    expect(scrollTo(cols, 1, 5000, 800)).toBeLessThanOrEqual(maxScroll(cols, 800));
    expect(scrollTo(cols, 0, 300, 800)).toBe(0);
  });
  it("shares a column's height by weight", () => {
    const h = heightsOf([w("a", 1), w("b", 3)], 412);
    expect(h[0]! + h[1]! + GAP).toBeCloseTo(412);
    expect(h[1]! / h[0]!).toBeCloseTo(3);
  });
  it("drags a boundary within limits", () => {
    const ws = [w("a"), w("b")];
    const n = dragBoundary(ws, 0, 50, 412);
    expect(n[0]).toBeGreaterThan(1); expect(n[1]).toBeLessThan(1);
    expect(dragBoundary(ws, 0, 5000, 412)).toEqual([1, 1]);
  });
});
