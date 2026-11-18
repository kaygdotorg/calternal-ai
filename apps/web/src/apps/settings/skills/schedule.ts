/**
 * Schedules for automations, typed in plain words and shown back in plain words. The words are read by the palette's recogniser (`palette/recognise.ts`: "every weekday at 9", "every monday 8:30", "daily at 7:15", "monthly");
 * what it understood becomes a `Schedule`, its plain description ("every weekday at 9:00") and the systemd `OnCalendar` expression that is stored in the automation's file (the owner's rule: automation schedules are
 * systemd-timer expressions with a time zone). What systemd cannot say simply ("every 2 weeks") is refused with a reason and a suggestion instead of being guessed. Pure: the clock is handed in.
 */
import "../../../palette/phrases";
import { recognise } from "../../../palette/recognise";
import { clockNow } from "../../../palette/dates";

/** When an automation runs: on these weekdays (0 = Sunday; null = every day) or on this day of the month, at this time of day. */
export interface Schedule { days: number[] | null; monthDay: number | null; h: number; m: number }
/** What typing a schedule gives: the understood schedule with how it reads back and its stored form, or why not. */
export type Parsed = { ok: true; schedule: Schedule; human: string; onCalendar: string } | { ok: false; why: string };

const DAY = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const SD = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const pad = (n: number) => String(n).padStart(2, "0");
const ordinal = (n: number) => `${n}${[11, 12, 13].includes(n % 100) ? "th" : ({ 1: "st", 2: "nd", 3: "rd" } as Record<number, string>)[n % 10] ?? "th"}`;
const isWeekdays = (d: number[]) => d.length === 5 && [1, 2, 3, 4, 5].every((x) => d.includes(x));

/** The schedule as words: "every day at 9:00", "every weekday at 9:00", "every Monday at 8:30", "every month on the 1st at 9:00". */
export function describe(s: Schedule): string {
  const at = `${s.h}:${pad(s.m)}`;
  if (s.monthDay !== null) return `every month on the ${ordinal(s.monthDay)} at ${at}`;
  if (!s.days) return `every day at ${at}`;
  if (isWeekdays(s.days)) return `every weekday at ${at}`;
  if (s.days.length === 1) return `every ${DAY[s.days[0]!]} at ${at}`;
  return `every ${s.days.map((d) => DAY[d]).join(", ")} at ${at}`;
}

/** The systemd `OnCalendar` form: `Mon..Fri *-*-* 09:00:00`. */
export function onCalendar(s: Schedule): string {
  const t = `${pad(s.h)}:${pad(s.m)}:00`;
  if (s.monthDay !== null) return `*-*-${pad(s.monthDay)} ${t}`;
  if (!s.days) return `*-*-* ${t}`;
  if (isWeekdays(s.days)) return `Mon..Fri *-*-* ${t}`;
  return `${[...s.days].sort((a, b) => a - b).map((d) => SD[d]).join(",")} *-*-* ${t}`;
}

/** The time of day as typed after "at" ("9", "9:30", "5pm"): a bare hour is that hour (the recogniser would pick the next one ahead of now, which is wrong for a schedule). */
function clock(t: string): [number, number] | null {
  const m = /\bat\s+(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b/i.exec(t) ?? /\b(\d{1,2}):(\d{2})\s*(am|pm)?\b/i.exec(t) ?? /\b(\d{1,2})()\s*(am|pm)\b/i.exec(t);
  if (!m) return null;
  let h = Number(m[1]); const min = Number(m[2] || 0); const ap = m[3]?.toLowerCase();
  if (ap === "pm" && h < 12) h += 12; if (ap === "am" && h === 12) h = 0;
  return h > 23 || min > 59 ? null : [h, min];
}

/** Reads a typed schedule. `now` is the clock (for the recogniser). */
export function parseSchedule(text: string, now = clockNow()): Parsed {
  const t = text.trim();
  if (!t) return { ok: false, why: "Say when, for example: every weekday at 9:00." };
  const p = recognise(t, { now });
  const r = p.repeat;
  if (!r) return { ok: false, why: "I could not tell how often. Try: every day at 9:00, every weekday at 9:00 or every Monday at 8:30." };
  const at = clock(t) ?? (p.time ? (p.time.split(":").map(Number) as [number, number]) : null);
  if (!at) return { ok: false, why: "At what time? Add one, such as: at 9:00 or at 17:30." };
  if (r.every > 1) return { ok: false, why: `A schedule cannot repeat every ${r.every} ${r.unit}s. Say it weekly or daily, or make two automations.` };
  const [h, m] = at;
  let s: Schedule;
  if (r.unit === "day") s = { days: null, monthDay: null, h, m };
  else if (r.unit === "week" && r.weekdays) s = { days: [1, 2, 3, 4, 5], monthDay: null, h, m };
  else if (r.unit === "week" && r.on !== undefined) s = { days: [r.on], monthDay: null, h, m };
  else if (r.unit === "week") return { ok: false, why: "On which day? Try: every Monday at 8:30." };
  else s = { days: null, monthDay: Number(/\bon the (\d{1,2})(?:st|nd|rd|th)\b/i.exec(t)?.[1]) || 1, h, m };
  return { ok: true, schedule: s, human: describe(s), onCalendar: onCalendar(s) };
}

/** The next time after `from` that the schedule fires, in the device's time zone (a month day that a month lacks is skipped for that month). */
export function nextRun(s: Schedule, from: Date): Date {
  for (let i = 0; i < 400; i++) {
    const d = new Date(from.getFullYear(), from.getMonth(), from.getDate() + i, s.h, s.m, 0, 0);
    const dayOk = s.monthDay !== null ? d.getDate() === s.monthDay : !s.days || s.days.includes(d.getDay());
    if (dayOk && d.getTime() > from.getTime()) return d;
  }
  return new Date(from.getTime() + 86_400_000);
}

/** When it fires next, in words: "today at 17:30", "tomorrow at 9:00", "Monday at 9:00", or the date. */
export function nextWords(s: Schedule, from: Date): string {
  const n = nextRun(s, from), days = Math.round((new Date(n.getFullYear(), n.getMonth(), n.getDate()).getTime() - new Date(from.getFullYear(), from.getMonth(), from.getDate()).getTime()) / 86_400_000);
  const at = `${n.getHours()}:${pad(n.getMinutes())}`;
  return days === 0 ? `today at ${at}` : days === 1 ? `tomorrow at ${at}` : days < 7 ? `${DAY[n.getDay()]} at ${at}` : `${n.getDate()} ${n.toLocaleString("en", { month: "short" })} at ${at}`;
}
