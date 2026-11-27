/**
 * The one date and time formatter of the mockup. Every app writes a moment through these functions, so the same instant reads the same everywhere:
 *  - a clock time is 24-hour with a leading zero: "09:24";
 *  - a day in a LIST (a row, a table cell, a chip) is relative within a week of today ("Today", "Yesterday", "Tomorrow", then the weekday in three letters, "Wed"), and otherwise the day and month, "14 Nov"; always with a capital;
 *  - a day on a PAGE (a header, a message's stamp) is the whole thing, "Friday 20 November", with the time after a comma, "Friday 20 November, 10:12".
 * Today is fixed at Friday 20 November 2026, the date every fixture is written against (so a fixture that says "Today" and one that says "20 Nov" agree). Fixture only.
 */
export const TODAY = "2026-11-20";
const DAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"], MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const at = (iso: string) => new Date(`${iso}T12:00:00Z`);
/** Whole days from today to `iso` (negative: in the past). */
export const diff = (iso: string): number => Math.round((at(iso).getTime() - at(TODAY).getTime()) / 86400000);
/** "09:24" from "9:24", "09:24" or "9.24". */
export const time = (t: string): string => { const m = /^(\d{1,2})[:.](\d{2})$/.exec(t.trim()); return m ? `${m[1]!.padStart(2, "0")}:${m[2]}` : t; };
/** The weekday of a date, in three letters ("Fri") or whole. */
export const weekday = (iso: string, long = false): string => { const d = DAYS[at(iso).getUTCDay()]!; return long ? d : d.slice(0, 3); };
/** A day in a list: "Today", "Yesterday", "Tomorrow", "Wed" within six days, else "14 Nov". */
export function day(iso: string): string {
  const n = diff(iso); if (n === 0) return "Today"; if (n === -1) return "Yesterday"; if (n === 1) return "Tomorrow";
  if (Math.abs(n) <= 6) return weekday(iso);
  const d = at(iso); return `${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]!.slice(0, 3)}`;
}
/** A day on a page: "Friday 20 November". */
export function dayLong(iso: string): string { const d = at(iso); return `${DAYS[d.getUTCDay()]} ${d.getUTCDate()} ${MONTHS[d.getUTCMonth()]}`; }
/** A moment on a page: "Friday 20 November, 10:12". */
export const stamp = (iso: string, t?: string): string => (t ? `${dayLong(iso)}, ${time(t)}` : dayLong(iso));
/** The moment an item shows wherever a list, a board or a table draws it: a clock time stays a clock time, a dated item shows its date through the shared list day (so one item reads the same in every view), anything else its own text, normalised. */
export function itemWhen(i: { when?: string; date?: string }): string | undefined {
  if (i.when && /^\d{1,2}[:.]\d{2}$/.test(i.when)) return time(i.when);
  return i.date ? day(i.date) : norm(i.when);
}
/** Normalises a fixture's loose text: a leading capital, a padded clock time ("yesterday" to "Yesterday", "9:24" to "09:24"). */
export const norm = (s: string | undefined): string | undefined => (s === undefined ? s : /^\d{1,2}[:.]\d{2}$/.test(s) ? time(s) : s.length ? s[0]!.toUpperCase() + s.slice(1) : s);
