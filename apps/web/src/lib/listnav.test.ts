import { describe, expect, test } from "vitest";
import { isScrollKey, scrollTarget, stepIndex, wantsKey } from "@calternal/ui";

describe("list navigation arithmetic", () => {
  test("Up and Down move one row and stop at the ends; from nowhere Down is the first row and Up the last", () => {
    expect(stepIndex("ArrowDown", 3, 10)).toBe(4);
    expect(stepIndex("ArrowDown", 9, 10)).toBe(9);
    expect(stepIndex("ArrowUp", 0, 10)).toBe(0);
    expect(stepIndex("ArrowDown", -1, 10)).toBe(0);
    expect(stepIndex("ArrowUp", -1, 10)).toBe(9);
  });
  test("Home, End and the Page keys never move the selection (they scroll)", () => {
    for (const k of ["Home", "End", "PageUp", "PageDown"]) { expect(stepIndex(k, 4, 10)).toBe(-1); expect(isScrollKey(k)).toBe(true); }
    expect(isScrollKey("ArrowDown")).toBe(false);
  });
  test("scroll targets: Home the start, End the end, a Page a viewport less a tenth, clamped", () => {
    expect(scrollTarget("Home", 500, 400, 2000)).toBe(0);
    expect(scrollTarget("End", 0, 400, 2000)).toBe(1600);
    expect(scrollTarget("PageDown", 100, 400, 2000)).toBe(460);
    expect(scrollTarget("PageDown", 1500, 400, 2000)).toBe(1600);
    expect(scrollTarget("PageUp", 100, 400, 2000)).toBe(0);
    expect(scrollTarget("ArrowDown", 0, 400, 2000)).toBeNull();
  });
  test("an empty list, or another key, goes nowhere", () => {
    expect(stepIndex("ArrowDown", 0, 0)).toBe(-1);
    expect(stepIndex("a", 0, 5)).toBe(-1);
  });
  test("a modified key, a key in a field or an open menu is not the list's", () => {
    const base = { key: "ArrowDown", altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, defaultPrevented: false, target: document.body };
    expect(wantsKey(base)).toBe(true);
    expect(wantsKey({ ...base, altKey: true })).toBe(false);
    expect(wantsKey({ ...base, shiftKey: true })).toBe(false);
    expect(wantsKey({ ...base, target: document.createElement("input") })).toBe(false);
    expect(wantsKey({ ...base, key: "Enter" })).toBe(false);
  });
});
