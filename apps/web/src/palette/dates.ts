// Small date helpers for the Composer's phrases. Dates are ISO days ("2026-10-13"), times are "17:00".
// Nothing here reads the clock: the caller hands in `today` and `now` (see `clockNow`), so a test can say
// what day it is and a phrase means the same on every keystroke.

export type Now = { today: string; h: number; m: number };

/** The device's day and time, right now. */
export function clockNow(d = new Date()): Now {
  const p = (n: number) => String(n).padStart(2, "0");
  return { today: `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`, h: d.getHours(), m: d.getMinutes() };
}


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

/** The next weekday on or after `from` (N1: a bare weekday includes today). */
export const nextWeekday = (target: number, from: string) => addDays(from, (target - dow(from) + 7) % 7);

/** Monday of next week plus the weekday ("next friday" is the following week's, N1). */
export function followingWeek(target: number, from: string) {
  const mon = addDays(from, (8 - dow(from)) % 7 || 7);
  return addDays(mon, (target + 6) % 7);
}
