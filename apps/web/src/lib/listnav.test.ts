import { describe, expect, test } from "vitest";
import { pageSize, stepIndex, wantsKey } from "@calternal/ui";

describe("list navigation arithmetic", () => {
  test("Up and Down move one row and stop at the ends; from nowhere Down is the first row and Up the last", () => {
    expect(stepIndex("ArrowDown", 3, 10, 5)).toBe(4);
    expect(stepIndex("ArrowDown", 9, 10, 5)).toBe(9);
    expect(stepIndex("ArrowUp", 0, 10, 5)).toBe(0);
    expect(stepIndex("ArrowDown", -1, 10, 5)).toBe(0);
    expect(stepIndex("ArrowUp", -1, 10, 5)).toBe(9);
  });
  test("Home and End jump; Page moves a screenful, clamped", () => {
    expect(stepIndex("Home", 7, 10, 5)).toBe(0);
    expect(stepIndex("End", 2, 10, 5)).toBe(9);
    expect(stepIndex("PageDown", 2, 10, 5)).toBe(7);
    expect(stepIndex("PageDown", 8, 10, 5)).toBe(9);
    expect(stepIndex("PageUp", 3, 10, 5)).toBe(0);
  });
  test("an empty list, or another key, goes nowhere", () => {
    expect(stepIndex("ArrowDown", 0, 0, 5)).toBe(-1);
    expect(stepIndex("a", 0, 5, 5)).toBe(-1);
  });
  test("a page is a screenful less one row, never under one", () => {
    expect(pageSize(480, 48)).toBe(9);
    expect(pageSize(10, 48)).toBe(1);
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
