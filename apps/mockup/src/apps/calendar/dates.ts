/** Date helpers for the Calendar mockup. Days are ISO strings (`2026-11-20`) and all arithmetic is done in UTC at noon, so no time zone or daylight-saving change can move a day. The mockup's clock is fixed: it is Friday 20 November 2026, 11:20. */
export const TODAY = "2026-11-20", NOW = "11:20";
const DAY = 864e5;
export const parse = (s: string): Date => new Date(`${s}T12:00:00Z`);
export const iso = (d: Date): string => d.toISOString().slice(0, 10);
/** `s` moved by `n` days. */
export const add = (s: string, n: number): string => iso(new Date(parse(s).getTime() + n * DAY));
/** 0 for Monday to 6 for Sunday (weeks start on Monday). */
export const dow = (s: string): number => (parse(s).getUTCDay() + 6) % 7;
export const weekStart = (s: string): string => add(s, -dow(s));
/** The weeks that cover the month of `s`, Monday first, each seven days (leading and trailing days belong to the neighbouring months). */
export function monthGrid(s: string): string[][] {
  const first = `${s.slice(0, 7)}-01`, weeks: string[][] = []; let d = weekStart(first);
  do { weeks.push(Array.from({ length: 7 }, (_, i) => add(d, i))); d = add(d, 7); } while (d.slice(0, 7) <= s.slice(0, 7) && weeks.length < 6);
  return weeks;
}
export const addMonths = (s: string, n: number): string => { const d = parse(s), t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + n, 1, 12)); return iso(t).slice(0, 8) + String(Math.min(d.getUTCDate(), 28)).padStart(2, "0"); };
const f = (o: Intl.DateTimeFormatOptions) => (s: string) => parse(s).toLocaleDateString("en-GB", { timeZone: "UTC", ...o });
export const weekday = f({ weekday: "long" }), weekdayShort = f({ weekday: "short" }), dayMonth = f({ day: "numeric", month: "long" }), monthYear = f({ month: "long", year: "numeric" }), dayNum = f({ day: "numeric" }), longDate = f({ weekday: "long", day: "numeric", month: "long" }), shortMonth = f({ month: "short" });
export const mins = (t: string): number => { const [h, m] = t.split(":").map(Number); return h! * 60 + m!; };
export const hm = (m: number): string => `${String(Math.floor(m / 60)).padStart(2, "0")}:${String(m % 60).padStart(2, "0")}`;
/** "Today", "Tomorrow", "Yesterday" or the weekday and date. */
export const relative = (s: string): string => s === TODAY ? "Today" : s === add(TODAY, 1) ? "Tomorrow" : s === add(TODAY, -1) ? "Yesterday" : longDate(s);
