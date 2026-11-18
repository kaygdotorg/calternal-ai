import { describe, expect, test } from "vitest";
import { collapse, type PillWidths } from "../src/lib/fit";

const w = (full: number, compact = 44): PillWidths => ({ full, compact });
const ids = ["attach", "model", "perm", "where"] as const;
const widths = { attach: w(44), model: w(190), perm: w(150), where: w(240) };
const gap = 8;

describe("which pills give up their words", () => {
  test("nothing does while the row fits in full", () => {
    expect(collapse(ids, widths, 44 + 190 + 150 + 240 + 3 * gap, gap, ["where", "perm", "model"])).toEqual(new Set());
  });
  test("they give them up one at a time in the stated order as the room shrinks", () => {
    const full = 44 + 190 + 150 + 240 + 3 * gap;
    expect(collapse(ids, widths, full - 1, gap, ["where", "perm", "model"])).toEqual(new Set(["where"]));
    expect(collapse(ids, widths, full - 196, gap, ["where", "perm", "model"])).toEqual(new Set(["where"]));
    expect(collapse(ids, widths, full - 197, gap, ["where", "perm", "model"])).toEqual(new Set(["where", "perm"]));
    expect(collapse(ids, widths, 44 * 4 + 3 * gap, gap, ["where", "perm", "model"])).toEqual(new Set(["where", "perm", "model"]));
  });
  test("a pill that holds a chosen value keeps its words until every other pill has given up its own", () => {
    const need3 = 44 + 190 + 44 + 44 + 3 * gap; // model in full, the rest compact
    expect(collapse(ids, widths, need3, gap, ["where", "perm", "model"], ["model"])).toEqual(new Set(["where", "perm"]));
    expect(collapse(ids, widths, need3 - 1, gap, ["where", "perm", "model"], ["model"])).toEqual(new Set(["where", "perm", "model"]));
  });
  test("under the floor every pill that holds nothing chosen is icon only, whatever would fit", () => {
    expect(collapse(ids, widths, 300, gap, ["where", "perm", "model"], [], 400)).toEqual(new Set(["model", "perm", "where"]));
    expect(collapse(ids, widths, 400, gap, ["where", "perm", "model"], ["model"], 450)).toEqual(new Set(["perm", "where"]));
    expect(collapse(ids, widths, 400, gap, ["where", "perm", "model"], [], 0)).toEqual(new Set(["where", "perm"]));
  });
  test("a pill with no shorter form is never listed, and an id not named in `drop` goes last", () => {
    expect(collapse(ids, widths, 10, gap, ["where"]).has("attach")).toBe(false);
    expect(collapse(["a", "b"] as const, { a: w(100), b: w(100) }, 150, 0, []).size).toBe(1);
  });
});
