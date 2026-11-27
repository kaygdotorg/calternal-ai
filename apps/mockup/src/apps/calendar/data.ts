/**
 * Calendar's fixture data: four calendars, about thirty events around "today" (Friday 20 November 2026, 11:20), and the sidebar's collections. Invented; nothing here is real.
 * An event is a `event` item (so the shell's rows, cards and cells draw it too) plus the details in `EVENTS`. Items the shell already has (`e-0`, `e-1`, `e-2`: Dentist, Lisbon flight, Planning review) keep their ids and get their details here.
 */
import { def, ITEMS, mk, pick, type Item } from "../../data/collections";
import { add, dayMonth, mins, TODAY, weekdayShort } from "./dates";

export interface Cal { id: string; name: string; hue: number }
export const CALS: Cal[] = [{ id: "personal", name: "Personal", hue: 205 }, { id: "work", name: "Work", hue: 280 }, { id: "family", name: "Family", hue: 330 }, { id: "birthdays", name: "Birthdays", hue: 55 }];
export const calOf = (id: string): Cal => CALS.find((c) => c.id === id)!;
export type Rsvp = "going" | "maybe" | "declined" | "waiting";
export interface Ev {
  id: string; title: string; cal: string; /** Each day it falls on (a repeat or a trip lists them all). */ days: string[]; /** "09:30"; none for an all-day event. */ start?: string; end?: string;
  where?: string; guests: string[]; notes?: string; repeat?: string; alert?: string; /** Items elsewhere this is about (a mail, a chat): the event page's Related. */ related?: string[]; busy?: boolean;
}
const day = (n: number, m = 11): string => `2026-${String(m).padStart(2, "0")}-${String(n).padStart(2, "0")}`;
const E = (id: string, title: string, cal: string, days: string[], start: string | undefined, end: string | undefined, x: Partial<Ev> = {}): Ev => ({ id, title, cal, days, start, end, guests: [], ...x });

export const EVENTS: Ev[] = [
  E("ev-0", "Stand-up", "work", [day(20), day(23), day(24), day(25), day(26), day(27)], "09:30", "09:45", { where: "Video call", guests: ["Chloe Rossi", "Ivo Novak", "Dev Patel"], repeat: "Every weekday", alert: "5 minutes before" }),
  E("ev-1", "Design review", "work", [day(20)], "11:00", "12:00", { where: "The long room", guests: ["Chloe Rossi", "Ivo Novak"], notes: "Sidebar spacing, the card in the middle and the phone tab bar. Bring the captures.", related: ["mx-0"], alert: "10 minutes before" }),
  E("ev-13", "Lunch with Ivo", "personal", [day(20)], "12:30", "13:30", { where: "Café Central", guests: ["Ivo Novak"] }),
  E("ev-14", "Release call", "work", [day(20)], "14:30", "15:30", { where: "Video call", guests: ["Dev Patel", "Ivo Novak", "Chloe Rossi"], notes: "Go or no go for the Monday build." }),
  E("ev-15", "Pick up the parcel", "personal", [day(20)], "15:00", "15:30", { where: "Post office, Rua da Palma", busy: false }),
  E("e-0", "Dentist", "personal", [day(21)], "09:30", "10:15", { where: "Clínica do Chiado, Rua Garrett 63", guests: ["Dr. Sofia Ferreira"], related: ["mx-5"], alert: "1 day before", notes: "Cleaning and the check of the left side. Arrive ten minutes early." }),
  E("ev-9", "Lena's birthday", "birthdays", [day(21)], undefined, undefined, { guests: ["Lena Berg"] }),
  E("ev-2", "Walk with Hana and Eva", "family", [day(21)], "10:30", "12:30", { where: "Miradouro da Senhora do Monte", guests: ["Hana Sato", "Eva Sato"], related: ["m-2", "c-hana"], notes: "Bring the thermos. Hana has the photos from last time." }),
  E("ev-5", "Climbing", "personal", [day(22)], "17:00", "19:00", { where: "Boulder Lisboa, Marvila", guests: ["Ben Martin", "Jun Park"], repeat: "Every other Sunday" }),
  E("ev-6", "Call Femi about the lease", "personal", [day(22)], "19:30", "19:45", { related: ["m-3", "dr-0"], notes: "Three per cent is fine; ask about March for the new rent and about the parking space." }),
  E("ev-4", "Prep: the three bets", "work", [day(23)], "11:00", "12:00", { where: "Home", related: ["mx-4"] }),
  E("e-2", "Planning review", "work", [day(23)], "14:00", "15:30", { where: "The long room", guests: ["Chloe Rossi", "Ivo Novak", "Dev Patel"], related: ["mx-4"], notes: "Agenda: what shipped, what slipped, the three bets, hiring. Questions, not slides." , alert: "30 minutes before" }),
  E("ev-7", "Yoga", "personal", [day(23), day(25)], "18:30", "19:30", { where: "Studio Respira", repeat: "Mondays and Wednesdays" }),
  E("ev-8", "Eva's birthday", "birthdays", [day(24)], undefined, undefined, { guests: ["Eva Sato"] }),
  E("ev-3", "Dinner at Eva's", "family", [day(24)], "20:00", "22:30", { where: "Rua do Alecrim 9", guests: ["Eva Sato", "Hana Sato", "Ben Martin"], notes: "Bring dessert, not wine: Eva has plenty." }),
  E("ev-10", "Tax return with Anna", "work", [day(25)], "15:00", "16:00", { where: "Okafor & Daughters, Av. da Liberdade 110", guests: ["Anna Okafor"], related: ["mx-1", "c-paper"], notes: "Bring the missing receipts." }),
  E("ev-26", "Quarterly planning offsite: product, platform, hiring and an unreasonably long title that tests how a title behaves in a narrow cell", "work", [day(25)], "09:00", "17:00", { where: "Convento de São Francisco, Sala do Capítulo, Rua da Madalena 14, Lisbon", guests: ["Chloe Rossi", "Ivo Novak", "Dev Patel", "Anna Okafor", "Jun Park", "Ben Martin", "Hana Sato", "Eva Sato"], repeat: "Every three months", alert: "1 hour before", notes: "Two tracks in the morning (product, platform), lunch together, then hiring and the budget in the afternoon. Please read the pre-reading before the day; it is long, but section four is the one that matters. Dietary needs go to Chloe by Monday. We finish at five sharp because the room is booked for a wedding rehearsal at half past, and nobody wants to explain to a bride why a slide about build times is still on screen." }),
  E("ev-18", "Book club", "family", [day(5), day(26)], "19:30", "21:30", { where: "Hana's", guests: ["Hana Sato", "Eva Sato", "Lena Berg"], repeat: "Monthly, last Thursday" }),
  E("e-1", "Lisbon: flight", "personal", [day(27)], "07:10", "09:45", { where: "Humberto Delgado Airport, Terminal 1", related: ["m-0", "m-1"], alert: "3 hours before", notes: "Cabin bag only. Booking AB4K7Q." }),
  E("ev-16", "Lisbon trip", "personal", [day(27), day(28), day(29)], undefined, undefined, { where: "Alfama", related: ["m-1"] }),
  E("ev-11", "Lease renewal: reply to Femi", "personal", [day(30)], undefined, undefined, { related: ["m-3", "dr-0"] }),
  E("ev-19", "Passport office", "personal", [day(3)], "10:00", "10:30", { where: "Loja do Cidadão, Laranjeiras" }),
  E("ev-20", "Hana's housewarming", "family", [day(7)], "18:00", "22:00", { where: "Travessa do Poço 7", guests: ["Hana Sato", "Eva Sato", "Lena Berg"] }),
  E("ev-21", "Brunch with Ben", "personal", [day(14)], "11:00", "12:30", { where: "Copenhagen Coffee Lab", guests: ["Ben Martin"] }),
  E("ev-22", "Run in Monsanto", "personal", [day(15)], "09:00", "10:30", { where: "Parque Florestal de Monsanto" }),
  E("ev-23", "Q4 kick-off", "work", [day(2)], "10:00", "11:30", { where: "The long room", guests: ["Chloe Rossi", "Ivo Novak", "Dev Patel"] }),
  E("ev-24", "Lease: sign and return", "personal", [day(4, 12)], "10:00", "10:30", { related: ["m-3"] }),
  E("ev-12", "Boiler service", "personal", ["2027-03-09"], "08:00", "12:00", { where: "At home", guests: ["Femi Hansen"], notes: "Femi confirms the exact day the week before." }),
];
export const EV: Record<string, Ev> = Object.fromEntries(EVENTS.map((e) => [e.id, e]));
const STATUS: Rsvp[] = ["going", "going", "maybe", "waiting", "declined", "going", "waiting", "going"];
/** A guest's answer, fixed by their place on the list so the fixture does not change between loads. */
export const rsvp = (ev: Ev, i: number): Rsvp => ev.cal === "birthdays" ? "going" : STATUS[(i + ev.title.length) % STATUS.length]!;
/** The events of a day, all-day first, then by start. */
export const onDay = (d: string, hidden: Set<string> = new Set()): Ev[] => EVENTS.filter((e) => e.days.includes(d) && !hidden.has(e.cal)).sort((a, b) => (a.start ? mins(a.start) : -1) - (b.start ? mins(b.start) : -1));
export const when = (e: Ev): string => (e.start ? `${e.start}–${e.end}` : "All day");

// Items: the shell's rows, cards and cells draw these.
for (const e of EVENTS) {
  const first = e.days[0]!, it: Item = ITEMS[e.id] ?? mk("event", e.id, e.title);
  it.kind = "event"; it.title = e.title; it.sub = `${weekdayShort(first)} ${dayMonth(first)}${e.start ? `, ${e.start}` : ", all day"}`; it.when = e.start ?? "all day"; it.due = weekdayShort(first); it.date = first; it.hue = calOf(e.cal).hue;
}
const todays = EVENTS.filter((e) => e.days.includes(TODAY)).map((e) => ITEMS[e.id]!);
const upcoming = EVENTS.filter((e) => e.days.some((d) => d > TODAY && d <= add(TODAY, 8))).sort((a, b) => a.days.find((d) => d > TODAY)!.localeCompare(b.days.find((d) => d > TODAY)!) || (a.start ?? "").localeCompare(b.start ?? "")).map((e) => ITEMS[e.id]!);
const ofCal = (c: string): Item[] => EVENTS.filter((e) => e.cal === c).sort((a, b) => a.days[0]!.localeCompare(b.days[0]!)).map((e) => ITEMS[e.id]!);
def({ id: "cal-today", name: "Today", hue: 205, library: true, small: "list", big: "list", items: todays });
def({ id: "cal-upcoming", name: "Upcoming", hue: 205, library: true, small: "list", big: "list", items: upcoming });
for (const c of CALS) def({ id: `cal-${c.id}`, name: c.name, hue: c.hue, shut: c.id !== "personal", small: "list", big: "list", items: ofCal(c.id) });
void pick;
