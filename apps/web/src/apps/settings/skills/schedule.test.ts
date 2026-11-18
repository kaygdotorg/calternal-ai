import { describe, expect, it } from "vitest";
import { describe as words, nextRun, nextWords, onCalendar, parseSchedule } from "./schedule";

const now = { today: "2026-11-18", h: 10, m: 0 }; // a Wednesday
const ok = (t: string) => { const p = parseSchedule(t, now); if (!p.ok) throw new Error(`${t}: ${p.why}`); return p; };
describe("typing a schedule", () => {
  it("every weekday at 9 reads back as every weekday at 9:00 and is stored as systemd OnCalendar", () => {
    const p = ok("every weekday at 9"); expect(p.human).toBe("every weekday at 9:00"); expect(p.onCalendar).toBe("Mon..Fri *-*-* 09:00:00");
  });
  it("a weekday name, daily and monthly", () => {
    expect(ok("every monday at 8:30").human).toBe("every Monday at 8:30");
    expect(ok("every monday at 8:30").onCalendar).toBe("Mon *-*-* 08:30:00");
    expect(ok("daily at 7:15").human).toBe("every day at 7:15");
    expect(ok("monthly at 9").onCalendar).toBe("*-*-01 09:00:00");
  });
  it("says why when it cannot", () => {
    expect(parseSchedule("", now)).toMatchObject({ ok: false });
    expect((parseSchedule("every weekday", now) as { why: string }).why).toMatch(/time/);
    expect((parseSchedule("at 9", now) as { why: string }).why).toMatch(/how often/);
    expect((parseSchedule("every 2 weeks at 9", now) as { why: string }).why).toMatch(/weekly or daily/);
  });
});
describe("when it fires next", () => {
  const from = new Date(2026, 10, 18, 10, 0, 0); // Wed 18 Nov 10:00
  it("finds the next weekday time, today when it is still ahead", () => {
    const s = { days: [1, 2, 3, 4, 5], monthDay: null, h: 17, m: 30 };
    expect(nextWords(s, from)).toBe("today at 17:30");
    expect(nextWords({ ...s, h: 9, m: 0 }, from)).toBe("tomorrow at 9:00");
  });
  it("skips the weekend and finds a named day", () => {
    expect(nextRun({ days: [1, 2, 3, 4, 5], monthDay: null, h: 9, m: 0 }, new Date(2026, 10, 20, 12, 0)).getDay()).toBe(1);
    expect(nextWords({ days: [1], monthDay: null, h: 9, m: 0 }, from)).toBe("Monday at 9:00");
  });
  it("words and OnCalendar for a list of days", () => {
    const s = { days: [1, 3, 5], monthDay: null, h: 8, m: 0 };
    expect(words(s)).toBe("every Monday, Wednesday, Friday at 8:00"); expect(onCalendar(s)).toBe("Mon,Wed,Fri *-*-* 08:00:00");
  });
});
