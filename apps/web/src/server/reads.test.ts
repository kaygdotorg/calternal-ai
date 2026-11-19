import { describe, expect, test } from "vitest";
import { isRead } from "./client";
import contract from "../../../../contract/operations.json";

// The client sends a read before the session is confirmed and holds everything else. A read that is not recognised is held, and a start-up that waits for it stops (the warm-reload deadlock on `onboarding.state`):
// so the client's idea of a read is compared with the contract's (`mutates`) in both directions.
const ops = contract as { id: string; mutates: boolean }[];

describe("which operations may go before the session is confirmed", () => {
  test("every operation the contract says does not change anything is sent at once", () => {
    expect(ops.filter((o) => !o.mutates && !isRead(o.id)).map((o) => o.id)).toEqual([]);
  });
  test("no operation that changes something is ever sent early", () => {
    expect(ops.filter((o) => o.mutates && isRead(o.id)).map((o) => o.id)).toEqual([]);
  });
  test("the start-up read that deadlocked is one of them", () => { expect(isRead("onboarding.state")).toBe(true); });
});
