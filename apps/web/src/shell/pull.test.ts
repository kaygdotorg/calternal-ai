import { describe, expect, it } from "vitest";
import { parseMs, pickTarget, settleOpen, visible } from "./pull";

const none = { leftOpen: false, rightOpen: false, hasRight: true };
describe("what a swipe does", () => {
  it("right opens the left sidebar, left opens the right one", () => {
    expect(pickTarget(1, none)).toEqual({ side: "left", opening: true });
    expect(pickTarget(-1, none)).toEqual({ side: "right", opening: true });
  });
  it("the opposite swipe closes what is open", () => {
    expect(pickTarget(-1, { ...none, leftOpen: true })).toEqual({ side: "left", opening: false });
    expect(pickTarget(1, { ...none, rightOpen: true })).toEqual({ side: "right", opening: false });
  });
  it("nothing to do when the swipe points at a sidebar already open, or there is no right sidebar", () => {
    expect(pickTarget(1, { ...none, leftOpen: true })).toBeNull();
    expect(pickTarget(-1, { ...none, rightOpen: true })).toBeNull();
    expect(pickTarget(-1, { ...none, hasRight: false })).toBeNull();
  });
});
describe("following the finger", () => {
  it("the visible width tracks the pull one to one, clamped to the sidebar", () => {
    expect(visible({ side: "left", opening: true }, 0, 100, 240)).toBe(100);
    expect(visible({ side: "left", opening: true }, 0, 400, 240)).toBe(240);
    expect(visible({ side: "left", opening: true }, 0, -30, 240)).toBe(0);
    expect(visible({ side: "right", opening: true }, 0, -100, 340)).toBe(100);
    expect(visible({ side: "left", opening: false }, 240, -60, 240)).toBe(180);
  });
  it("release: a flick decides, else half", () => {
    expect(settleOpen(40, 240, 0.6)).toBe(true);
    expect(settleOpen(200, 240, -0.6)).toBe(false);
    expect(settleOpen(130, 240, 0)).toBe(true);
    expect(settleOpen(110, 240, 0.1)).toBe(false);
  });
});

describe("CSS times", () => {
  it("reads milliseconds and seconds, and falls back on anything else", () => {
    expect(parseMs("340ms", 1)).toBe(340);
    expect(parseMs(" 0.19s ", 1)).toBeCloseTo(190);
    expect(parseMs("200", 1)).toBe(200);
    expect(parseMs("", 7)).toBe(7);
    expect(parseMs("var(--x)", 7)).toBe(7);
  });
});
