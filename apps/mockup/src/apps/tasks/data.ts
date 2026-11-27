/**
 * Fixture data of the Tasks app: invented tasks, projects and the collections its sidebar draws. Nothing here is real or saved.
 * A task is an `Item` of kind "task" (the shell's type); what an Item has no field for (priority, repeat, tags, subtasks, the note it came from) is in `X`, by the task's id.
 * "Today" in the fixture is Friday 20 November 2026, so the lists are the same whenever the mockup is opened.
 */
import { ITEMS, def, mk, type Item } from "../../data/collections";

export const TODAY = "2026-11-20";
export type Pri = 1 | 2 | 3 | 4;
/** Everything a task has beyond the shell's Item. `when` is the day it is planned for (ISO), `time` its hour, `deadline` the day it must be done by. */
export interface X { project: string; when?: string; time?: string; deadline?: string; pri?: Pri; repeat?: string; tags: string[]; note?: string; waiting?: string; desc?: string; subs?: [string, boolean][]; log?: string[] }
export const PROJECTS = [
  { id: "trip", name: "Trip to Lisbon", hue: 205 }, { id: "home", name: "Home", hue: 28 }, { id: "work", name: "Work", hue: 280 },
  { id: "errands", name: "Errands", hue: 150 }, { id: "calternal", name: "Calternal", hue: 330 }, { id: "garden", name: "Garden", hue: 120 },
];
export const projectOf = (name: string) => PROJECTS.find((p) => p.name === name) ?? PROJECTS[1]!;
export const PRIORITY: Record<Pri, string> = { 4: "Urgent", 3: "High", 2: "Medium", 1: "Low" };

const DAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
import { day, dayLong as dayLongShared } from "../../lib/when";
const utc = (iso: string) => Date.parse(`${iso}T00:00:00Z`);
/** Whole days from the fixture's today to `iso` (negative: past). */
export const diff = (iso: string) => Math.round((utc(iso) - utc(TODAY)) / 86_400_000);
/** The shared list day (lib/when.ts): "Today", "Tomorrow", "Yesterday", a weekday inside a week, else "3 Dec". */
export const dayName = day;
/** A day as a heading (lib/when.ts): "Saturday 21 November". */
export const dayLong = dayLongShared;

export const X: Record<string, X> = {};
type Row = [id: string, title: string, status: Item["status"], x: X, owner?: string];
const rows: Row[] = [
  ["tk-landlord", "Call the landlord about the lease", "To do", { project: "Home", when: "2026-11-19", pri: 3, tags: [], desc: "Ask whether the renewal can start in January, and whether the rent stays the same. Ask about the balcony door too." }],
  ["tk-passport", "Renew passport", "Doing", { project: "Errands", when: TODAY, time: "10:00", pri: 3, deadline: "2026-12-04", tags: ["admin"], note: "nt-lisbon", subs: [["Find the form", true], ["Take the photos", true], ["Book the appointment", false], ["Pay the fee", false]], desc: "The office takes walk-ins before noon. Bring the old passport and two photos, 35 by 45 mm.", log: ["You started this · Wed", "You ticked “Find the form” · Wed"] }, "Ada"],
  ["tk-invoice", "Send the invoice to the studio", "To do", { project: "Work", when: TODAY, pri: 4, deadline: TODAY, tags: ["money"], desc: "Hours for November, with the two extra days. Copy the accountant." }, "Ivo"],
  ["tk-update", "Write the Friday update", "To do", { project: "Work", when: TODAY, time: "16:00", repeat: "Every Friday", tags: [], log: ["Done last Fri", "Done 6 Nov", "Skipped 30 Oct"] }, "Ada"],
  ["tk-water", "Water the plants", "To do", { project: "Home", when: TODAY, repeat: "Every 3 days", tags: ["quick"] }],
  ["tk-charger", "Pack the charger and the adapter", "To do", { project: "Trip to Lisbon", when: "2026-11-21", tags: ["quick"], note: "nt-packing" }],
  ["tk-tickets", "Book the museum tickets", "To do", { project: "Trip to Lisbon", when: "2026-11-21", pri: 2, tags: [], note: "nt-lisbon", desc: "The tile museum first, then the one by the river. Both open at ten; the second is closed on Mondays." }, "Hana"],
  ["tk-hotel", "Confirm the check-in time with the hotel", "To do", { project: "Trip to Lisbon", when: "2026-11-22", tags: [], waiting: "Hotel Alfama", note: "nt-lisbon" }],
  ["tk-tram", "Buy a 24-hour tram pass", "To do", { project: "Trip to Lisbon", when: "2026-11-23", tags: ["quick"], note: "nt-lisbon" }],
  ["tk-spacing", "Review Ada's sidebar spacing change", "To do", { project: "Calternal", when: "2026-11-23", pri: 2, tags: [], note: "nt-ada" }, "Ivo"],
  ["tk-icon", "Draw the missing tag icon", "To do", { project: "Calternal", when: "2026-11-24", tags: ["design"], note: "nt-ada" }, "Ada"],
  ["tk-receipts", "Scan the receipts for the accountant", "To do", { project: "Errands", when: "2026-11-25", deadline: "2026-12-01", pri: 2, tags: ["admin", "money"], subs: [["Hotel deposit", false], ["Train tickets", false], ["Café Central", false]] }],
  ["tk-bulb", "Replace the hallway bulb", "To do", { project: "Home", tags: ["quick"] }],
  ["tk-book", "Return the library book", "To do", { project: "Errands", tags: ["quick"] }],
  ["tk-gift", "Think of a gift for Hana", "To do", { project: "Home", tags: [], desc: "She likes ceramics and anything for the balcony." }],
  ["tk-bike", "Book the bike service", "To do", { project: "Home", pri: 1, tags: [] }],
  ["tk-article", "Read the long article on type", "To do", { project: "Work", tags: ["later"], note: "nt-long" }],
  ["tk-flights", "Book the flights", "Done", { project: "Trip to Lisbon", when: "2026-11-17", tags: [], note: "nt-lisbon" }],
  ["tk-adapter", "Order the plug adapter", "Done", { project: "Trip to Lisbon", when: "2026-11-18", tags: ["quick"] }],
  ["tk-deposit", "Pay the hotel deposit", "Done", { project: "Trip to Lisbon", when: "2026-11-12", pri: 3, tags: ["money"] }],
];
export const tasks: Item[] = rows.map(([id, title, status, x, owner]) => {
  X[id] = x;
  const label = x.when ? dayName(x.when) : undefined;
  return mk("task", id, title, x.project, { status, due: label, date: x.when, owner, hue: projectOf(x.project).hue });
});
/** A task the shell made up (a mixed collection's) has no entry: it lands in the Inbox. */
export const xOf = (id: string): X => X[id] ?? { project: "Inbox", tags: [] };
export const isDone = (st?: string) => st === "Done";

/** Counts the sidebar shows (fixture: they are what the lists below hold on 20 November). */
export const open = (t: Item) => t.status !== "Done";
export const lists = {
  today: () => tasks.filter((t) => open(t) && xOf(t.id).when && diff(xOf(t.id).when!) <= 0),
  upcoming: () => tasks.filter((t) => open(t) && xOf(t.id).when && diff(xOf(t.id).when!) > 0),
  anytime: () => tasks.filter((t) => open(t) && !xOf(t.id).when),
  done: () => tasks.filter((t) => !open(t)),
};
const count = (id: keyof typeof lists) => lists[id]().length;

/** A label row opens a collection of what it counts: a list or a project as a big collection in a window (the shell's `opens`). */
const opens = (label: Item, id: string, name: string, items: Item[], big: "list" | "board" = "list"): Item => { def({ id, name, hue: 28, small: "list", big, items }); label.opens = id; return label; };
export const collections = [
  def({ id: "tk-smart", name: "Lists", hue: 28, small: "list", big: "list", items: [
    opens(mk("label", "tks-today", "Today", "", { icon: "sun", count: count("today") }), "tk-today", "Today", lists.today(), "board"), opens(mk("label", "tks-upcoming", "Upcoming", "", { icon: "calendar", count: count("upcoming") }), "tkc-upcoming", "Upcoming", lists.upcoming()),
    opens(mk("label", "tks-anytime", "Anytime", "", { icon: "list", count: count("anytime") }), "tkc-anytime", "Anytime", lists.anytime()), opens(mk("label", "tks-done", "Completed", "", { icon: "check" }), "tkc-done", "Completed", lists.done()),
  ] }),
  def({ id: "tk-today", name: "Today", hue: 28, library: true, small: "list", big: "board", items: lists.today() }),
  def({ id: "tk-projects", name: "Projects", hue: 28, small: "list", big: "list", items: PROJECTS.map((p) => opens(mk("label", `tkp-${p.id}`, p.name, "", { hue: p.hue, count: tasks.filter((t) => open(t) && t.sub === p.name).length || undefined }), `tkc-${p.id}`, p.name, tasks.filter((t) => t.sub === p.name), "board")) }),
  def({ id: "tk-waiting", name: "Waiting on", hue: 28, library: true, small: "list", big: "list", items: tasks.filter((t) => xOf(t.id).waiting) }),
];
