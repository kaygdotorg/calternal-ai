// Small date helpers for the Composer's phrases. Dates are ISO days ("2026-10-13"), times are "17:00".
// Nothing here reads the clock: the caller hands in `today` and `now` (see `clockNow`), so a test can say
// what day it is and a phrase means the same on every keystroke.

export type Now = { today: string; h: number; m: number };

/** The device's day and time, right now. */
export function clockNow(d = new Date()): Now {
  const p = (n: number) => String(n).padStart(2, "0");
  return { today: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, h: d.getHours(), m: d.getMinutes() };
}

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const parseIso = (s: string) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y!, m! - 1, d!, 12);
};
const toIso = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
export const addDays = (s: string, n: number) => {
  const d = parseIso(s);
  d.setDate(d.getDate() + n);
  return toIso(d);
};
export const dow = (s: string) => parseIso(s).getDay();
export const diffDays = (a: string, b: string) => Math.round((parseIso(a).getTime() - parseIso(b).getTime()) / 86_400_000);
export const dayName = (i: number) => DAYS[i]!;
export const monthName = (i: number) => MONTHS[i]!;
const monthOf = (s: string) => parseIso(s).getMonth();
const dayOf = (s: string) => parseIso(s).getDate();

/** The next weekday on or after `from` (N1: a bare weekday includes today). */
export const nextWeekday = (target: number, from: string) => addDays(from, (target - dow(from) + 7) % 7);

/** Monday of next week plus the weekday ("next friday" is the following week's, N1). */
export function followingWeek(target: number, from: string) {
  const mon = addDays(from, (8 - dow(from)) % 7 || 7);
  return addDays(mon, (target + 6) % 7);
}

/** "5 pm", "5:30 pm". */
export function fmtTime(t: string) {
  const [h, m] = t.split(":").map(Number);
  const hh = h! % 12 || 12;
  return `${hh}${m ? `:${String(m).padStart(2, "0")}` : ""} ${h! < 12 ? "am" : "pm"}`;
}

/** A day the short way: Today, Tomorrow, Fri (within a week), else "24 Oct". */
export function fmtDay(s: string, today: string) {
  const d = diffDays(s, today);
  if (d === 0) return "Today";
  if (d === 1) return "Tomorrow";
  if (d === -1) return "Yesterday";
  if (d > 1 && d < 7) return dayName(dow(s));
  return `${dayOf(s)} ${monthName(monthOf(s))}`;
}
