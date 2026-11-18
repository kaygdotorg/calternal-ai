// The Selector's motion and measuring rules (ported from the old app's primitives/selector tests, unchanged in substance): which capsule edge leads, that a move stretches and then gathers, how keys skip disabled options and wrap, and that
// the selectors mounted together read every option before any capsule writes a width.
import { describe, expect, test } from "vitest";
import { edgeConfigs, leadingEdge, step, LEAD, TRAIL } from "../../../packages/ui/src/components/selector/edges";
import { measurementBatch } from "../../../packages/ui/src/components/selector/measure-batch";
import { Spring } from "../../../packages/ui/src/lib/spring-engine";

describe("selector capsule edges", () => {
  test("the edge on the side it's moving toward leads", () => {
    expect(leadingEdge({ left: 0, right: 50 }, { left: 100, right: 160 })).toBe("right");
    expect(leadingEdge({ left: 100, right: 160 }, { left: 0, right: 50 })).toBe("left");
    expect(leadingEdge({ left: 0, right: 50 }, { left: 0, right: 52 })).toBe("right");
    expect(leadingEdge({ left: 0, right: 50 }, { left: 0, right: 50 })).toBeNull();
    expect(edgeConfigs({ left: 0, right: 50 }, { left: 100, right: 160 })).toEqual({ left: TRAIL, right: LEAD });
  });
  test("moving right, the capsule stretches first, then gathers (it never inverts)", () => {
    const from = { left: 0, right: 60 }, to = { left: 200, right: 260 };
    const cfg = edgeConfigs(from, to);
    const L = new Spring(from.left, cfg.left, 0.1), R = new Spring(from.right, cfg.right, 0.1);
    L.target = to.left; R.target = to.right;
    let widest = 0;
    for (let i = 0; i < 90; i++) { L.step(1 / 60); R.step(1 / 60); widest = Math.max(widest, R.value - L.value); expect(R.value - L.value).toBeGreaterThan(0); }
    expect(widest).toBeGreaterThan(90);
    expect(L.value).toBe(200);
    expect(R.value).toBe(260);
  });
  test("a reversal mid-flight decides the lead again from where it is", () => {
    expect(leadingEdge({ left: 80, right: 170 }, { left: 0, right: 60 })).toBe("left");
  });
  test("keys skip disabled options and wrap", () => {
    const disabled = [false, true, false, false];
    expect(step(disabled, 0, 1)).toBe(2);
    expect(step(disabled, 3, 1)).toBe(0);
    expect(step(disabled, 0, -1)).toBe(3);
    expect(step(disabled, -1, 1)).toBe(0);
    expect(step(disabled, 4, -1)).toBe(3);
    expect(step([true, true], 0, 1)).toBe(0);
  });
});

describe("selector measuring", () => {
  test("mounting selectors reads every option before any capsule changes layout", () => {
    const scheduled: (() => void)[] = [];
    const batch = measurementBatch((flush) => scheduled.push(flush));
    const events: string[] = [];
    batch(() => events.push("read a"), () => events.push("paint a"));
    batch(() => events.push("read b"), () => events.push("paint b"));
    expect(scheduled).toHaveLength(1);
    scheduled.shift()!();
    expect(events).toEqual(["read a", "read b", "paint a", "paint b"]);
  });
  test("an unmounted selector neither measures nor paints, including during a flush", () => {
    const scheduled: (() => void)[] = [];
    const batch = measurementBatch((flush) => scheduled.push(flush));
    const events: string[] = [];
    const cancel = batch(() => events.push("gone read"), () => events.push("gone paint"));
    cancel();
    let cancelDuringRead = () => {};
    batch(() => cancelDuringRead(), () => events.push("live paint"));
    cancelDuringRead = batch(() => events.push("cancelled read"), () => events.push("cancelled paint"));
    scheduled.shift()!();
    expect(events).toEqual(["live paint"]);
  });
  test("a new measurement requested while painting runs in the next batch", () => {
    const scheduled: (() => void)[] = [];
    const batch = measurementBatch((flush) => scheduled.push(flush));
    const events: string[] = [];
    batch(() => events.push("read"), () => { events.push("paint"); batch(() => events.push("next read"), () => events.push("next paint")); });
    scheduled.shift()!();
    expect(events).toEqual(["read", "paint"]);
    scheduled.shift()!();
    expect(events).toEqual(["read", "paint", "next read", "next paint"]);
  });
});
