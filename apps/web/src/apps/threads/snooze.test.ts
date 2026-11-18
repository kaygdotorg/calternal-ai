import { describe, expect, it } from "vitest";
import { backLabel, snoozeChoices } from "./snooze";

const at = (y: number, m: number, d: number, h: number, min = 0) => new Date(y, m - 1, d, h, min).getTime();

describe("snoozeChoices", () => {
  it("offers later today three hours on, to the quarter hour, tomorrow 09:00 and next Monday 09:00", () => {
    const c = snoozeChoices(at(2026, 11, 18, 10, 7)); // a Wednesday
    expect(c.map((x) => x.id)).toEqual(["today", "tomorrow", "week"]);
    expect(new Date(c[0]!.at).getHours()).toBe(13);
    expect(new Date(c[0]!.at).getMinutes()).toBe(15);
    expect(c[1]!.label).toBe("Tomorrow, 09:00");
    expect(new Date(c[2]!.at).getDay()).toBe(1);
    expect(new Date(c[2]!.at).getDate()).toBe(23);
  });
  it("drops later today when three hours on is tomorrow", () => {
    expect(snoozeChoices(at(2026, 11, 18, 22, 0)).map((x) => x.id)).toEqual(["tomorrow", "week"]);
  });
  it("next week is never tomorrow: on a Sunday it is the Monday after tomorrow's", () => {
    const sunday = at(2026, 11, 22, 10); // Sunday; tomorrow is Monday the 23rd
    const c = snoozeChoices(sunday);
    expect(new Date(c[1]!.at).getDate()).toBe(23);
    expect(new Date(c[2]!.at).getDate()).toBe(30);
  });
});

describe("backLabel", () => {
  const now = at(2026, 11, 18, 10);
  it("says the time today, tomorrow, or the date", () => {
    expect(backLabel(at(2026, 11, 18, 14, 30), now)).toBe("Back at 14:30");
    expect(backLabel(at(2026, 11, 19, 9), now)).toBe("Back tomorrow, 09:00");
    expect(backLabel(at(2026, 11, 23, 9), now)).toBe("Back Mon 23 Nov, 09:00");
  });
});
