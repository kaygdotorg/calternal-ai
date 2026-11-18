import { describe, expect, it } from "vitest";
import { WheelSwipe, parseMs, pickTarget, settleOpen, visible } from "./pull";

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
describe("a trackpad swipe", () => {
  const ev = (deltaX: number, deltaY: number, t: number) => ({ deltaX, deltaY, deltaMode: 0, timeStamp: t });
  it("starts on the first horizontal event, follows the stream and ends after a quiet moment", () => {
    const w = new WheelSwipe();
    expect(w.feed(ev(-10, 1, 0), () => true)).toBe("start");
    expect(w.feed(ev(-20, 0, 16), () => true)).toBe("take");
    expect(w.dx).toBe(30);
    expect(w.velocity()).toBeGreaterThan(0);
    expect(w.feed(ev(-10, 0, 400), () => true)).toBe("start"); // a new swipe after the pause
    expect(w.dx).toBe(10);
  });
  it("every delta counts however the events are batched: one large event, many small ones and lines give the same travel", () => {
    const one = new WheelSwipe(); one.feed(ev(-100, 0, 0), () => true);
    const many = new WheelSwipe(); for (let i = 0; i < 25; i++) many.feed(ev(-4, 0, i * 16), () => true);
    const lines = new WheelSwipe(); for (let i = 0; i < 25; i++) lines.feed({ deltaX: -0.25, deltaY: 0, deltaMode: 1, timeStamp: i * 16 }, () => true);
    const pages = new WheelSwipe(); pages.feed({ deltaX: -0.25, deltaY: 0, deltaMode: 2, timeStamp: 0 }, () => true);
    expect([one.dx, many.dx, lines.dx, pages.dx]).toEqual([100, 100, 100, 100]);
  });
  it("a vertical scroll and a swipe handed to a sideways scroller are left alone for the whole stream", () => {
    const w = new WheelSwipe();
    expect(w.feed(ev(2, 30, 0), () => true)).toBe("leave");
    expect(w.feed(ev(-30, 3, 16), () => true)).toBe("leave");
    const v = new WheelSwipe();
    expect(v.feed(ev(-30, 0, 0), () => false)).toBe("leave");
    expect(v.feed(ev(-30, 0, 16), () => true)).toBe("leave");
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
