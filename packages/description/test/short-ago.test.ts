import { describe, expect, it } from "vitest";
import { shortAgo } from "../src/render/eval";

describe("shortAgo (the time on a one-line row)", () => {
  const now = new Date(2026, 10, 20, 15, 0, 0).getTime();
  const ago = (ms: number) => shortAgo(now - ms, now);
  it("is as short as a row can hold", () => {
    expect(ago(20_000)).toBe("now");
    expect(ago(5 * 60_000)).toBe("5m");
    expect(ago(60 * 60_000)).toBe("1h");
    expect(ago(6 * 3600_000)).toBe("6h");
    expect(shortAgo(new Date(2026, 10, 19, 23, 0).getTime(), now)).toBe("yesterday");
    expect(ago(3 * 86400_000)).toBe("3d");
    expect(ago(14 * 86400_000)).toBe("2w");
  });
  it("never takes more than nine characters", () => { for (const d of [1, 59, 61, 3599, 7200, 90000, 200000, 700000, 3e6, 9e7, 9e9]) expect(ago(d * 1000).length).toBeLessThanOrEqual(9); });
});
