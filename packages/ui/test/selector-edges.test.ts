import { describe, expect, test } from "vitest";
import { edgeConfigs, leadingEdge, step, LEAD, TRAIL } from "../src/components/selector/edges";
import { Spring } from "../src/lib/spring-engine";

describe("selector capsule edges", () => {
  test("the edge on the side it's moving toward leads", () => {
    expect(leadingEdge({ left: 0, right: 50 }, { left: 100, right: 160 })).toBe("right");
    expect(leadingEdge({ left: 100, right: 160 }, { left: 0, right: 50 })).toBe("left");
    expect(leadingEdge({ left: 0, right: 50 }, { left: 0, right: 52 })).toBe("right");
    expect(leadingEdge({ left: 0, right: 50 }, { left: 0, right: 50 })).toBeNull();
    expect(edgeConfigs({ left: 0, right: 50 }, { left: 100, right: 160 })).toEqual({ left: TRAIL, right: LEAD });
  });

  test("moving right, the capsule stretches first, then gathers (it never inverts)", () => {
    const from = { left: 0, right: 60 };
    const to = { left: 200, right: 260 };
    const cfg = edgeConfigs(from, to);
    const L = new Spring(from.left, cfg.left, 0.1);
    const R = new Spring(from.right, cfg.right, 0.1);
    L.target = to.left;
    R.target = to.right;
    let widest = 0;
    for (let i = 0; i < 90; i++) {
      L.step(1 / 60);
      R.step(1 / 60);
      widest = Math.max(widest, R.value - L.value);
      expect(R.value - L.value).toBeGreaterThan(0);
    }
    expect(widest).toBeGreaterThan(90); // it really stretched
    expect(L.value).toBe(200);
    expect(R.value).toBe(260);
  });

  test("a reversal mid-flight decides the lead again from where it is", () => {
    // Halfway to the right, then back to the start: now the left edge leads.
    expect(leadingEdge({ left: 80, right: 170 }, { left: 0, right: 60 })).toBe("left");
  });

  test("keys skip disabled options and wrap", () => {
    const disabled = [false, true, false, false];
    expect(step(disabled, 0, 1)).toBe(2);
    expect(step(disabled, 3, 1)).toBe(0);
    expect(step(disabled, 0, -1)).toBe(3);
    expect(step(disabled, -1, 1)).toBe(0); // Home
    expect(step(disabled, 4, -1)).toBe(3); // End
    expect(step([true, true], 0, 1)).toBe(0);
  });
});
