/**
 * Snoozing a thread (docs/threads-ui.md, "Snoozed"): the times the menu offers, from the clock now. Pure, so the rules are testable. A snoozed thread leaves the Inbox and is back at the time; the person can also pick a date.
 */
/** One time the snooze menu offers. */
export interface SnoozeChoice { id: "today" | "tomorrow" | "week"; label: string; at: number }

const two = (n: number) => String(n).padStart(2, "0");
const clock = (d: Date) => `${two(d.getHours())}:${two(d.getMinutes())}`;
const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** The presets from `now`: later today (three hours on, rounded up to the quarter hour, only while that is still today), tomorrow at 09:00, and next Monday at 09:00 (always after tomorrow's, so "next week" is never today or tomorrow). */
export function snoozeChoices(now: number): SnoozeChoice[] {
  const d = new Date(now);
  const out: SnoozeChoice[] = [];
  const later = new Date(now + 3 * 3600_000);
  later.setMinutes(Math.ceil(later.getMinutes() / 15) * 15, 0, 0);
  if (later.getDate() === d.getDate()) out.push({ id: "today", label: `Later today, ${clock(later)}`, at: later.getTime() });
  const tomorrow = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1, 9, 0, 0, 0);
  out.push({ id: "tomorrow", label: `Tomorrow, ${clock(tomorrow)}`, at: tomorrow.getTime() });
  const ahead = ((8 - d.getDay()) % 7) || 7; // days to the next Monday, never 0
  const monday = new Date(d.getFullYear(), d.getMonth(), d.getDate() + (ahead === 1 ? ahead + 7 : ahead), 9, 0, 0, 0);
  out.push({ id: "week", label: `Next week, ${DAYS[monday.getDay()]} ${clock(monday)}`, at: monday.getTime() });
  return out;
}

/** The short form for a one-line row: the time today ("13:00"), "Tomorrow", else the date ("Wed 25 Nov"). The full phrase is `backLabel`, kept as the row's tooltip. */
export function backShort(at: number, now: number): string {
  const d = new Date(at), n = new Date(now);
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(d) - day(n)) / 86_400_000);
  return diff <= 0 ? clock(d) : diff === 1 ? "Tomorrow" : `${DAYS[d.getDay()]} ${d.getDate()} ${d.toLocaleString("en-GB", { month: "short" })}`;
}

/** When a snoozed thread returns, in the words a row shows: "Back at 14:30" today, "Back tomorrow, 09:00", else "Back Mon 24 Nov, 09:00". */
export function backLabel(at: number, now: number): string {
  const d = new Date(at), n = new Date(now);
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(d) - day(n)) / 86_400_000);
  if (diff <= 0) return `Back at ${clock(d)}`;
  if (diff === 1) return `Back tomorrow, ${clock(d)}`;
  return `Back ${DAYS[d.getDay()]} ${d.getDate()} ${d.toLocaleString("en-GB", { month: "short" })}, ${clock(d)}`;
}
