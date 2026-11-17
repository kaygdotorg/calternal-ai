// The words that name a day, in one registry so a language pack can add its own (C21). A word maps to a
// function from today to an ISO day. Phrases that are not single words ("in 3 days", "24 oct") stay in
// phrases-tasks.ts; this is only the part a language changes.
import { addDays, followingWeek, nextWeekday } from "./dates";

export type DayWord = (today: string) => string;

const words = new Map<string, DayWord>();
let version = 0;

const WD = "(?:mon(?:day)?|tue(?:s|sday)?|wed(?:nesday)?|thu(?:r|rs|rsday)?|fri(?:day)?|sat(?:urday)?|sun(?:day)?)";
export const WEEKDAY = WD;
export const wdIndex = (s: string) => ["sun", "mon", "tue", "wed", "thu", "fri", "sat"].indexOf(s.slice(0, 3).toLowerCase());

/** Teaches the recogniser a day word. Lower case; may hold spaces ("agle hafte"). */
export function addDayWord(word: string, fn: DayWord): void {
  words.set(word.toLowerCase(), fn);
  version++;
}

addDayWord("today", (t) => t);
addDayWord("tonight", (t) => t);
addDayWord("tomorrow", (t) => addDays(t, 1));
addDayWord("tmrw", (t) => addDays(t, 1));
addDayWord("next week", (t) => followingWeek(1, t));

/** The day a single day word means, or null. */
export function dayWord(w: string, today: string): string | null {
  return words.get(w)?.(today) ?? null;
}

/** The weekday forms: "friday", "this friday" (the coming one), "next friday" (the following week's, N1). */
export function weekdayDay(phrase: string, today: string): string | null {
  const m = phrase.match(new RegExp(`^(this |next )?(${WD})$`));
  if (!m) return null;
  return m[1] === "next " ? followingWeek(wdIndex(m[2]!), today) : nextWeekday(wdIndex(m[2]!), today);
}

let cached: { v: number; alt: string } | null = null;
/** A regex alternative for every day word, longest first so "next week" beats "next". */
export function dayWordAlt(): string {
  if (cached?.v === version) return cached.alt;
  const alt = [...words.keys()]
    .sort((a, b) => b.length - a.length)
    .map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&").replace(/ /g, "\\s+"))
    .join("|");
  cached = { v: version, alt };
  return alt;
}
export const dayWordsVersion = () => version;
