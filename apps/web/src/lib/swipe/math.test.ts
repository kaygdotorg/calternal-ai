import { describe, expect, it } from "vitest";
import { band, fullAt, sideOf, takeover, trayWidth, SLOT } from "./math";
import { claimsSwipe } from "./start";
import { springTo } from "./spring";

const two = { n: 2, full: true, leaves: true };

describe("swipe numbers (the existing app's)", () => {
  it("follows the finger one to one up to the buttons, then with resistance", () => {
    expect(band(100, two)).toBe(100);
    expect(trayWidth(two)).toBe(2 * SLOT);
    expect(band(-200, two)).toBeCloseTo(-(128 + 72 * 0.55));
    expect(sideOf(-1)).toBe(-1);
  });
  it("commits a long swipe past the buttons plus 56 px, or 55 % of the row", () => {
    expect(fullAt(two, 300)).toBe(184);
    expect(fullAt(two, 400)).toBeCloseTo(220);
    expect(takeover(two, 300, 100)).toBe(0);
  });
  it("claims only a quick, mostly sideways move", () => {
    expect(claimsSwipe(12, 3, 100)).toBe(true);
    expect(claimsSwipe(12, 8, 100)).toBe(false);
    expect(claimsSwipe(12, 3, 400)).toBe(false);
    expect(claimsSwipe(8, 0, 50)).toBe(false);
  });
  it("a spring that starts at speed ends at rest", () => {
    const s = springTo(0, -130, -1);
    expect(s.duration).toBeGreaterThan(100);
    expect(s.easing.startsWith("linear(")).toBe(true);
  });
});
