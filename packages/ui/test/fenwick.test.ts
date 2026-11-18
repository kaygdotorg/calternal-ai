import { describe, expect, it } from "vitest";
import { Fenwick } from "../src/lib/fenwick";

describe("Fenwick", () => {
  const h = [10, 20, 30, 40, 50];
  it("prefix sums", () => {
    const f = new Fenwick(h);
    expect([0, 1, 2, 5].map((i) => f.prefix(i))).toEqual([0, 10, 30, 150]);
  });
  it("finds the row holding an offset, clamped at both ends", () => {
    const f = new Fenwick(h);
    expect(f.find(0)).toBe(0);
    expect(f.find(9.9)).toBe(0);
    expect(f.find(10)).toBe(1);
    expect(f.find(59)).toBe(2);
    expect(f.find(60)).toBe(3);
    expect(f.find(10_000)).toBe(4);
    expect(f.find(-5)).toBe(0);
  });
  it("updates one height", () => {
    const f = new Fenwick(h);
    f.add(1, 5);
    expect(f.prefix(2)).toBe(35);
    expect(f.find(34)).toBe(1);
    expect(f.find(35)).toBe(2);
  });
  it("matches brute force on 10,000 random rows", () => {
    let seed = 7;
    const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296);
    const rows = Array.from({ length: 10_000 }, () => 20 + Math.floor(rnd() * 400));
    const f = new Fenwick(rows);
    for (let k = 0; k < 200; k++) {
      const i = Math.floor(rnd() * rows.length);
      const d = Math.floor(rnd() * 50) - 20;
      rows[i] += d;
      f.add(i, d);
    }
    let run = 0;
    const starts = rows.map((r) => ((run += r), run - r));
    for (let k = 0; k < 200; k++) {
      const off = rnd() * run;
      let want = 0;
      while (want + 1 < rows.length && starts[want + 1]! <= off) want++;
      expect(f.find(off)).toBe(want);
      expect(f.prefix(want)).toBeCloseTo(starts[want]!, 6);
    }
  });
  it("handles an empty list", () => {
    const f = new Fenwick([]);
    expect(f.prefix(0)).toBe(0);
    expect(f.find(10)).toBe(0);
  });
});
