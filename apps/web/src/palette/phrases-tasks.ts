// The Tasks phrases (C11): dates, times, repeats, deadlines and priority, read out of a sentence. They follow
// the #590 date rules: a bare weekday is the next one including today (N1), a bare "5" is guessed from the
// time of day and never asked (N2). A date makes a task, "by friday" is a Deadline, "every monday" repeats.
// Obsidian's emoji (T2) are read too, so a line written there still means the same. Everything is a regex
// built once; a keystroke runs each of them once.
import { addDays, diffDays, dow, nextWeekday, type Now } from "./dates";
import { dayWord, dayWordAlt, dayWordsVersion, WEEKDAY as WD, weekdayDay, wdIndex } from "./date-words";
import "./phrases-hinglish";
import { registerPhrases, type Priority, type Reader, type Repeat } from "./recognise";

const MON = "(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*";
const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const pad = (n: number) => String(n).padStart(2, "0");
const hhmm = (h: number, m = 0) => `${pad(h)}:${pad(m)}`;
const to24 = (h: number, ap?: string) => (ap === "pm" ? (h % 12) + 12 : ap === "am" ? h % 12 : h);

// The date alternatives depend on the day words, which a language pack can extend: rebuilt when they change.
let built = -1;
let DATE_RE: RegExp;
let BY_RE: RegExp;
function dateRes() {
  if (built !== dayWordsVersion()) {
    const alt = `${dayWordAlt()}|(?:(?:this|next)\\s+)?${WD}|in\\s+\\d+\\s+(?:days?|weeks?)|\\d{4}-\\d{2}-\\d{2}|\\d{1,2}(?:st|nd|rd|th)?\\s+${MON}|${MON}\\s+\\d{1,2}(?:st|nd|rd|th)?`;
    DATE_RE = new RegExp(`(?<![\\p{L}\\d])(?:${alt})(?![\\p{L}\\d])`, "giu");
    BY_RE = new RegExp(`(?<![\\p{L}\\d])by\\s+(?:${alt})(?![\\p{L}\\d])`, "giu");
    built = dayWordsVersion();
  }
  return { DATE_RE, BY_RE };
}
const EVERY_RE = new RegExp(`\\bevery\\s+(?:(\\d+)\\s+(day|week|month)s?|(day|week|month|weekday)|(${WD}))\\b|\\b(daily|weekly|monthly)\\b`, "gi");
const RANGE_RE = /\b(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\s*(?:-|–|—|to)\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b/gi;
const TIME_RE = /\b(?:at\s+)?(\d{1,2})(?::(\d{2}))?\s*(am|pm)\b|\b(\d{1,2}):(\d{2})\b/gi;
const AT_RE = /\bat\s+(\d{1,2})\b(?!\s*(?::|am|pm))/gi;
const BARE_AFTER = new RegExp(`^\\s+(\\d{1,2})(?![\\d:]|\\s*(?:am|pm|st|nd|rd|th|${MON})\\b)`, "i");
const PRIO_RE = /!(urgent|high|medium|med|low)(?![\p{L}\d])|(?<![\w!])(!{1,3})(?![\w!])|(?<![\p{L}\d])urgent(?![\p{L}\d])/giu;
const EMOJI_PRIO = /[🔺⏫🔼🔽]/gu;
const EMOJI_DATE = /([📅⏳])\s*(\d{4}-\d{2}-\d{2})/gu;

/** One date phrase to an ISO day (N1). */
export function dateOf(phrase: string, today: string): string | null {
  const p = phrase.toLowerCase().replace(/\s+/g, " ").trim();
  const w = dayWord(p, today) ?? weekdayDay(p, today);
  if (w) return w;
  let m = p.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (m) return Number(m[2]) >= 1 && Number(m[2]) <= 12 && Number(m[3]) >= 1 && Number(m[3]) <= 31 ? p : null;
  m = p.match(/^in (\d+) (day|days|week|weeks)$/);
  if (m) return addDays(today, Number(m[1]) * (m[2]!.startsWith("week") ? 7 : 1));
  const mi = (s: string) => MONTHS.indexOf(s.slice(0, 3));
  let day = 0;
  let mon = -1;
  if ((m = p.match(new RegExp(`^(\\d{1,2})(?:st|nd|rd|th)? (${MON})$`)))) [day, mon] = [Number(m[1]), mi(m[2]!)];
  else if ((m = p.match(new RegExp(`^(${MON}) (\\d{1,2})(?:st|nd|rd|th)?$`)))) [day, mon] = [Number(m[2]), mi(m[1]!)];
  if (mon < 0 || day < 1 || day > 31) return null;
  const y = Number(today.slice(0, 4));
  let iso = `${y}-${pad(mon + 1)}-${pad(day)}`;
  if (diffDays(iso, today) < 0) iso = `${y + 1}-${pad(mon + 1)}-${pad(day)}`;
  return iso;
}

/** N2: a bare hour is guessed from the time of day, never asked. */
function guessHour(n: number, date: string | null, now: Now): number {
  if (n >= 13 || n === 0) return n % 24;
  const am = n % 12;
  const pm = am + 12;
  if (!date || date === now.today) return am * 60 > now.h * 60 + now.m ? am : pm;
  return n === 12 ? 12 : n <= 6 ? pm : am;
}

const VERBS = /^(call|buy|email|text|pay|book|send|remind|renew|order|pick|clean|water|return|cancel|submit|reply|schedule|fix|finish|ring|message|todo)\b/i;
const QUESTION = /^(how|what|where|when|why|who|which|did|do|does|is|are|can|could|should|find|show|search)\b|\?\s*$/i;

function read(r: Reader) {
  // A line read as a contact keeps its words: a phone number is not a time.
  if (r.extra.contact) return;
  const { text, now, slots } = r;
  const { DATE_RE, BY_RE } = dateRes();
  // Obsidian's emoji: 📅 is the Deadline, ⏳ the When (TQ8 draws our own icons; the emoji are only read).
  r.each(EMOJI_DATE, (m) => {
    const emoji = m[1] === "📅" ? "deadline" : "date";
    if (r.claim(m.index, m.index + m[0].length, emoji)) {
      if (emoji === "deadline") slots.deadline ??= m[2]!;
      else slots.when ??= m[2]!;
    }
  });
  r.each(EMOJI_PRIO, (m) => {
    const p: Priority = ({ "🔺": 4, "⏫": 3, "🔼": 2, "🔽": 1 } as Record<string, Priority>)[m[0]]!;
    if (r.claim(m.index, m.index + m[0].length, "priority") && p > slots.priority) slots.priority = p;
  });
  // "by friday": a Deadline, not a When.
  r.each(BY_RE, (m) => {
    const d = dateOf(m[0].replace(/^by\s+/i, ""), now.today);
    if (d && !slots.deadline && r.claim(m.index, m.index + m[0].length, "deadline")) slots.deadline = d;
  });
  // "every monday", "daily", "every 2 weeks".
  r.each(EVERY_RE, (m) => {
    if (slots.repeat) return;
    let rep: Repeat | null = null;
    if (m[1]) rep = { every: Number(m[1]), unit: m[2]!.toLowerCase() as Repeat["unit"] };
    else if (m[3]) rep = m[3].toLowerCase() === "weekday" ? { every: 1, unit: "week", weekdays: true } : { every: 1, unit: m[3].toLowerCase() as Repeat["unit"] };
    else if (m[4]) rep = { every: 1, unit: "week", on: wdIndex(m[4]) };
    else if (m[5]) rep = { every: 1, unit: ({ daily: "day", weekly: "week", monthly: "month" } as const)[m[5].toLowerCase() as "daily"] };
    if (rep && r.claim(m.index, m.index + m[0].length, "repeat")) slots.repeat = rep;
  });
  r.each(/🔁/gu, (m) => void r.claim(m.index, m.index + m[0].length, "repeat"));
  // A time range, "1–2 pm".
  let range = false;
  r.each(RANGE_RE, (m) => {
    if (range || !(m[3] || m[6] || m[2] || m[5])) return;
    const e = to24(Number(m[4]), m[6]?.toLowerCase());
    const ap1 = m[3]?.toLowerCase();
    const h1 = Number(m[1]);
    const s = ap1 ? to24(h1, ap1) : to24(h1, m[6]?.toLowerCase()) <= e ? to24(h1, m[6]?.toLowerCase()) : to24(h1, "am");
    if (h1 > 12 && !m[3] && !m[6]) return;
    if (r.claim(m.index, m.index + m[0].length, "time")) {
      range = true;
      slots.time = hhmm(s, Number(m[2] ?? 0));
      slots.endTime = hhmm(e, Number(m[5] ?? 0));
    }
  });
  // A time: "5 pm", "17:00".
  r.each(TIME_RE, (m) => {
    if (slots.time) return;
    const h = m[4] ? Number(m[4]) : to24(Number(m[1]), m[3]!.toLowerCase());
    const mi = Number(m[5] ?? m[2] ?? 0);
    if (h < 24 && mi < 60 && r.claim(m.index, m.index + m[0].length, "time")) slots.time = hhmm(h, mi);
  });
  // Dates. The first one is the When.
  r.each(DATE_RE, (m) => {
    const d = dateOf(m[0], now.today);
    if (d && r.claim(m.index, m.index + m[0].length, "date") && !slots.when) slots.when = d;
  });
  // "fri 5": a bare number after a date is an hour (N2).
  if (!slots.time) {
    const last = r.spans.filter((s) => s.type === "date").sort((a, b) => b.end - a.end)[0];
    const hit = last && text.slice(last.end).match(BARE_AFTER);
    if (hit && r.claim(last.end + hit[0].indexOf(hit[1]!), last.end + hit[0].length, "time")) slots.time = hhmm(guessHour(Number(hit[1]), slots.when, now));
  }
  if (!slots.time) {
    r.each(AT_RE, (m) => {
      if (!slots.time && r.claim(m.index, m.index + m[0].length, "time")) slots.time = hhmm(guessHour(Number(m[1]), slots.when, now));
    });
  }
  r.each(PRIO_RE, (m) => {
    const word = m[1]?.toLowerCase();
    const p: Priority = word ? ({ urgent: 4, high: 3, medium: 2, med: 2, low: 1 } as Record<string, Priority>)[word]! : m[2] === "!" ? 3 : 4;
    if (r.claim(m.index, m.index + m[0].length, "priority") && p > slots.priority) slots.priority = p;
  });
  // #tags and @people are metadata too (C22). Not inside a word or a link: they start at a boundary.
  r.each(/(?<![\p{L}\d\w/&])#([\p{L}][\p{L}\d_/-]*)/gu, (m) => {
    if (r.claim(m.index, m.index + m[0].length, "tag")) slots.tags.push(m[1]!);
  });
  r.each(/(?<![\p{L}\d\w.])@([\p{L}][\p{L}\d_.-]*[\p{L}\d_])/gu, (m) => {
    if (r.claim(m.index, m.index + m[0].length, "person")) slots.people.push(m[1]!);
  });
  // A repeat or a time with no day lands on the first day it fits.
  if (!slots.when && slots.repeat) slots.when = slots.repeat.on != null ? nextWeekday(slots.repeat.on, now.today) : slots.repeat.weekdays && [0, 6].includes(dow(now.today)) ? nextWeekday(1, now.today) : now.today;
  if (!slots.when && slots.time && !slots.deadline) slots.when = slots.time > hhmm(now.h, now.m) ? now.today : addDays(now.today, 1);
}

function vote(r: Reader) {
  const t = r.text.trim();
  const s = r.slots;
  const metadata = !!(s.when || s.deadline || s.repeat || s.priority || s.tags.length || s.people.length);
  if (r.checkbox) return metadata ? ("task" as const) : null;
  if (QUESTION.test(t)) return "open" as const;
  if (metadata || VERBS.test(t)) return "task" as const;
  // A tie (C19): plain words typed in the Tasks app are a task; anywhere else the next plugin decides.
  return r.here === "tasks" && t ? ("task" as const) : null;
}

registerPhrases({ id: "tasks", read, vote });
