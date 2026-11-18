import { expect, test } from "vitest";
import { measurementBatch } from "../src/components/selector/measure-batch";

test("mounting selectors reads every option before any capsule changes layout", () => {
  const scheduled: (() => void)[] = [];
  const batch = measurementBatch((flush) => scheduled.push(flush));
  const events: string[] = [];
  batch(() => events.push("read a"), () => events.push("paint a"));
  batch(() => events.push("read b"), () => events.push("paint b"));
  expect(scheduled).toHaveLength(1);
  expect(events).toEqual([]);
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
  batch(() => events.push("read"), () => {
    events.push("paint");
    batch(() => events.push("next read"), () => events.push("next paint"));
  });
  scheduled.shift()!();
  expect(events).toEqual(["read", "paint"]);
  scheduled.shift()!();
  expect(events).toEqual(["read", "paint", "next read", "next paint"]);
});
