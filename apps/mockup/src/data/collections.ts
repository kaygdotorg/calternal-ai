/**
 * Fixture data of the mockup's collections. A collection is a named, ordered set of items of any kind, with a small view (a list or a strip, drawn in a sidebar section) and a big view (list, table, board or gallery, drawn in a window).
 * A sidebar is an ordered list of collections: Chat's Groups, People and pinned strip are collections, and so is Mail's "Recent notes" once a person drags it there. Invented people and things; nothing here is real.
 */
import { day, norm, weekday } from "../lib/when";
export type Kind = "note" | "mail" | "chat" | "task" | "file" | "photo" | "event" | "contact" | "transaction" | "workout" | "label" | "setting";
/** An item. Which fields are used depends on its kind: the look of each kind is in `shell/Item.svelte`. */
export interface Item { id: string; kind: Kind; title: string; sub?: string; when?: string; n?: number; muted?: boolean; hue?: number; amount?: string; status?: "To do" | "Doing" | "Done"; due?: string; icon?: string; count?: number; folder?: boolean; cal?: boolean; unread?: boolean; /** The first lines of a note or document (a Craft-style page thumbnail shows them). */ body?: string; /** For an agent's chat: whose model it is (its mark replaces initials). */ provider?: "claude" | "codex" | "gemini" | "local"; /** ISO date, for the calendar view and sorting. */ date?: string; owner?: string; /** Opening this item opens a collection (a collection id, or `sb:<sidebar>`) instead of the item's own page: a label row such as Today, a folder or a #tag. */ opens?: string }
export type SmallView = "list" | "strip";
export type BigView = "list" | "table" | "board" | "gallery";
export interface Coll { id: string; name: string; hue: number; /** In the card's Collections tab (the library) and so draggable into a sidebar. */ library?: boolean; small: SmallView; big: BigView; /** Drawn without a title row (Chat's pinned strip). */ bare?: boolean; /** Starts folded. */ shut?: boolean; /** A pinned strip: dropping a row on it pins (copies), dragging a row off it unpins. */ pin?: boolean; items: Item[] }

/** The helpers an app uses to declare its fixture data (see apps/README in the mockup README): `mk` an item, `seq` a run of items, `pick` items already declared, `def` a collection. */
/** Every item by id, so a collection can name an item another one also holds. */
export const ITEMS: Record<string, Item> = {};
export const reg = (i: Item): Item => (ITEMS[i.id] = i);
export const mk = (kind: Kind, id: string, title: string, sub?: string, x: Partial<Item> = {}): Item => reg({ kind, id, title, sub, ...x });
export const COLLS: Record<string, Coll> = {};
export const def = (c: Omit<Coll, "items"> & { items: Item[] }): Coll => (COLLS[c.id] = c);
export const pick = (...ids: string[]): Item[] => ids.map((i) => ITEMS[i]!);
export const seq = (kind: Kind, id: string, rows: [string, string?, Partial<Item>?][]): Item[] => rows.map(([t, s, x], i) => mk(kind, `${id}${i}`, t, s, x));

// Chat
const chats = {
  lunch: mk("chat", "c-lunch", "Where to eat on Friday", "River it is. Booking now.", { when: "09:24", n: 3 }), release: mk("chat", "c-release", "Release notes", "Draft is in the doc", { when: "08:10", provider: "claude" }), ada: mk("chat", "c-ada", "Ada Lindqvist", "See you at six", { when: "Yesterday" }),
  ben: mk("chat", "c-ben", "Ben Okafor", "Can you look at the PR?", { when: "09:02", n: 4 }), chloe: mk("chat", "c-chloe", "Chloe Martin", "Thanks!", { when: "Mon" }), dev: mk("chat", "c-dev", "Dev Patel", "Sent the invoice", { when: "10:40", n: 1 }), eva: mk("chat", "c-eva", "Eva Sato", "Lovely, see you", { when: "Sun" }),
  spacing: mk("chat", "c-spacing", "Sidebar spacing", "Let's try eight pixels", { when: "Yesterday", n: 2, muted: true }), paper: mk("chat", "c-paper", "Paperwork for the accountant", "Receipts are in the folder", { when: "23 Oct" }),
  lisbon: mk("chat", "c-lisbon", "Trip to Lisbon: dates", "The 14th works for me", { when: "3d" }), hana: mk("chat", "c-hana", "Hana Sato", "Photos from the walk", { when: "9h" }), ivo: mk("chat", "c-ivo", "Ivo Novak", "Did the build pass?", { when: "11h", n: 1 }), jun: mk("chat", "c-jun", "Jun Park", "Ok!", { when: "12h" }),
  rc: mk("chat", "c-rc", "Release captain", "Waiting for a request", { cal: true, provider: "claude" }), rs: mk("chat", "c-rs", "Researcher", "Waiting for a request", { cal: true, provider: "claude" }), audit: mk("chat", "c-audit", "Dependency audit", "Three advisories found", { when: "11h", provider: "codex" }),
  flat: mk("chat", "c-flat", "Moving flat: checklist", "Keys returned", { when: "25 Sep" }), demo: mk("chat", "c-demo", "Demo script", "Version two", { when: "14 Aug", provider: "codex" }),
};
def({ id: "chat-pins", name: "Pinned", hue: 40, small: "strip", big: "gallery", bare: true, pin: true, items: pick("c-lunch", "c-release", "c-ada", "c-ben", "c-chloe", "c-dev", "c-eva") });
def({ id: "groups", name: "Groups", hue: 205, small: "list", big: "list", items: pick("c-spacing", "c-paper") });
def({ id: "people", name: "People", hue: 330, small: "list", big: "list", items: pick("c-lisbon", "c-hana", "c-ivo", "c-jun") });
def({ id: "cals", name: "Cals", hue: 280, small: "list", big: "list", items: pick("c-rc", "c-rs") });
def({ id: "harnesses", name: "Harnesses", hue: 150, small: "list", big: "list", items: pick("c-audit") });
def({ id: "archived", name: "Archived", hue: 0, small: "list", big: "list", shut: true, items: pick("c-flat", "c-demo") });
void chats;

// Notes, mail, tasks
const BODY = ["Flights are booked and the hotel is still open. Check the tram pass and the museum hours.", "The plan for the three regions: web first, then the Mac app, then the phone.", "Typing speed is back to 62 words a minute after the second week. Home row first.", "Soak the beans overnight. Eight hours on low, then cream and butter at the end.", "Affordances and signifiers: a door that needs a sign has been designed wrongly.", "Decisions and who does what. Ada takes the interface, Ivo the build, Hana the copy.", "A very long note about everything that came up in the long read last week."];
const notes = seq("note", "n-", [["Autumn trip", "Flights are booked; hotel still open", { when: "Today" }], ["Calternal rewrite", "The plan for the three regions", { when: "Mon" }], ["Colemak, week three", "Typing speed back to 62 wpm", { when: "Sun" }], ["Dal makhani", "Soak overnight; 8 hours on low", { when: "Sat" }], ["Design of everyday things", "Affordances and signifiers", { when: "Fri" }], ["Meeting with Ada", "Decisions and who does what", { when: "Thu" }], ["Long read", "A very long note", { when: "Wed" }]]);
def({ id: "notes-pinned", name: "Pinned", hue: 55, pin: true, small: "strip", big: "list", items: notes.slice(0, 2) });
def({ id: "recent-notes", name: "Recent notes", hue: 55, library: true, small: "list", big: "list", items: notes.slice(2, 7).concat(notes.slice(0, 1)) });
def({ id: "tags", name: "Tags", hue: 150, small: "list", big: "list", items: seq("label", "tag-", [["#travel", "", { count: 6, hue: 205 }], ["#recipes", "", { count: 14, hue: 28 }], ["#work", "", { count: 22, hue: 280 }]]) });
const mails = seq("mail", "m-", [["Airline", "Your booking is confirmed", { when: "09:41", unread: true }], ["Hotel Alfama", "Check-in instructions", { when: "08:15", unread: true }], ["Hana Sato", "Photos from the walk", { when: "Yesterday" }], ["Landlord", "About the lease renewal", { when: "Mon", unread: true }], ["Bank", "Your statement is ready", { when: "Sun" }], ["Café Central", "Receipt for 14 Nov", { when: "Sat" }]]);
def({ id: "recent-mail", name: "Recent mail", hue: 205, library: true, small: "list", big: "list", items: mails });
def({ id: "mailboxes", name: "Mailboxes", hue: 205, small: "list", big: "list", items: seq("label", "mb-", [["Inbox", "", { count: 4, icon: "mail" }], ["Starred", "", { icon: "spark" }], ["Drafts", "", { count: 2, icon: "edit" }], ["Sent", "", { icon: "send" }], ["Archive", "", { icon: "archive" }]]) });
def({ id: "labels", name: "Labels", hue: 150, small: "list", big: "list", items: seq("label", "lb-", [["Receipts", "", { hue: 150 }], ["Travel", "", { hue: 205 }], ["Family", "", { hue: 330 }]]) });
const tasks = seq("task", "t-", [["Book the museum tickets", "Trip to Lisbon", { due: "Mon", status: "To do" }], ["Renew passport", "Errands", { due: "Tue", status: "Doing" }], ["Send the invoice", "Work", { due: "Wed", status: "To do" }], ["Call the landlord", "Home", { due: "Thu", status: "Done" }], ["Pack the charger", "Trip to Lisbon", { due: "Fri", status: "To do" }]]);
def({ id: "tasks-week", name: "Tasks due this week", hue: 28, library: true, small: "list", big: "board", items: tasks });
def({ id: "t-smart", name: "Smart lists", hue: 28, small: "list", big: "list", items: seq("label", "ts-", [["Today", "", { count: 5, icon: "clock" }], ["Upcoming", "", { count: 12, icon: "list" }], ["Flagged", "", { count: 2, icon: "spark" }], ["Completed", "", { icon: "check" }]]) });
def({ id: "t-lists", name: "My lists", hue: 28, small: "list", big: "list", items: seq("label", "tl-", [["Home", "", { count: 7, hue: 28 }], ["Work", "", { count: 9, hue: 280 }], ["Errands", "", { count: 3, hue: 150 }]]) });
def({ id: "unread-chats", name: "Unread chats", hue: 330, library: true, small: "list", big: "list", items: pick("c-lunch", "c-ben", "c-spacing", "c-ivo", "c-dev") });

// Files, photos, calendar, contacts, settings
def({ id: "f-fav", name: "Favourites", hue: 205, small: "list", big: "list", items: seq("file", "ff-", [["Documents", "", { folder: true }], ["Downloads", "", { folder: true, count: 3 }], ["Pictures", "", { folder: true }]]) });
def({ id: "f-loc", name: "Locations", hue: 175, small: "list", big: "list", items: seq("label", "fl-", [["This computer", "", { icon: "terminal" }], ["Home space", "", { icon: "files" }], ["Shared with me", "", { icon: "share", count: 2 }]]) });
def({ id: "f-recent", name: "Recent files", hue: 205, small: "list", big: "table", items: seq("file", "fr-", [["Lease 2026.pdf", "412 KB", { when: "Yesterday" }], ["Budget.xlsx", "88 KB", { when: "Monday" }], ["Scan 0412.png", "2.1 MB", { when: "Last week" }]]) });
def({ id: "p-lib", name: "Library", hue: 330, small: "list", big: "list", items: seq("label", "pl-", [["All photos", "", { count: 12840, icon: "image" }], ["Favourites", "", { count: 212, icon: "spark" }], ["Recents", "", { icon: "clock" }]]) });
def({ id: "p-albums", name: "Albums", hue: 330, small: "strip", big: "gallery", items: seq("photo", "pa-", [["Lisbon", "142 photos", { hue: 205 }], ["Moving day", "88 photos", { hue: 28 }], ["Bike", "31 photos", { hue: 175 }], ["Family", "310 photos", { hue: 330 }], ["Recipes", "67 photos", { hue: 12 }]]) });
def({ id: "p-people", name: "People", hue: 330, small: "strip", big: "gallery", items: seq("contact", "pp-", [["Ada", "", {}], ["Ben", "", {}], ["Lena", "", {}], ["Hana", "", {}]]) });
notes.forEach((n, i) => { n.body = BODY[i]; n.date = `2026-11-${20 - i}`; });
tasks.forEach((t, i) => { t.date = `2026-11-${23 + i}`; t.owner = ["Ada", "Ivo", "Ada", "Hana", "Jun"][i]; });
mails.forEach((m, i) => { m.date = `2026-11-${20 - i}`; });
const events = seq("event", "e-", [["Dentist", "Tomorrow, 09:30", { when: "09:30", due: "Tomorrow" }], ["Lisbon: flight", "Friday, 07:10", { when: "07:10", due: "Fri" }], ["Planning review", "Monday, 14:00", { when: "14:00", due: "Mon" }]]);
events.forEach((e, i) => { e.date = ["2026-11-21", "2026-11-27", "2026-11-23"][i]; });
def({ id: "cal-cals", name: "Calendars", hue: 205, small: "list", big: "list", items: seq("label", "cc-", [["Personal", "", { hue: 205 }], ["Work", "", { hue: 280 }], ["Family", "", { hue: 330 }], ["Birthdays", "", { hue: 55 }]]) });
def({ id: "cal-up", name: "Coming up", hue: 205, small: "list", big: "list", items: events });
def({ id: "c-groups", name: "Groups", hue: 330, small: "list", big: "list", items: seq("label", "cg-", [["Everyone", "", { count: 84, icon: "user" }], ["Family", "", { count: 12, icon: "user" }], ["Work", "", { count: 31, icon: "user" }]]) });
const people = ["Ada Lindqvist", "Anna Okafor", "Ben Martin", "Chloe Rossi", "Dev Patel", "Eva Sato", "Femi Hansen"].map((n, i) => mk("contact", `cp-${i}`, n, ["Neighbour", "Accountant", "Friend", "Colleague", "Colleague", "Cousin", "Landlord"][i]));
def({ id: "c-people", name: "People", hue: 330, small: "list", big: "gallery", items: people });
def({ id: "settings-list", name: "Settings", hue: 0, small: "list", big: "list", items: seq("setting", "st-", [["Appearance", "", { icon: "sun" }], ["Machines", "", { icon: "terminal" }], ["Accounts", "", { icon: "user" }], ["Plugins", "", { icon: "apps" }], ["Notifications", "", { icon: "bell" }], ["Privacy", "", { icon: "shield" }]]) });
const money = seq("transaction", "x-", [["Hotel deposit", "Hotel Alfama", { amount: "-€240.00", when: "12 Nov" }], ["Train tickets", "CP", { amount: "-€38.50", when: "10 Nov" }], ["Refund", "Airline", { amount: "+€64.00", when: "9 Nov" }]]);
const runs = seq("workout", "w-", [["Morning run", "5.2 km", { when: "31:04", icon: "pulse" }], ["Hike in Sintra", "11.8 km", { when: "3:12 h" }]]);
void money; void runs;

// Mixed collections (the library): each draws every item with its kind's look.
const POOL: Record<string, [string, string][]> = {
  note: [["Packing list", "Passport, charger, adapter"], ["Ideas for day two", "Alfama in the morning"], ["Notes from the call", "Decisions and next steps"]], mail: [["Flight confirmation", "Airline"], ["Hotel booking", "Hotel Alfama"], ["Receipt, café", "Café Central"]],
  chat: [["Thread: who drives?", "Ivo"], ["Dinner plans", "Hana"]], task: [["Book the tickets", "Trip to Lisbon"], ["Renew passport", "Errands"], ["Call the landlord", "Home"]], file: [["Budget.xlsx", "88 KB"], ["Passport scan.pdf", "1.2 MB"], ["Lease 2026.pdf", "412 KB"]],
  photo: [["Photo 0412", "Alfama"], ["Sunset", "Cabo da Roca"], ["Map of the old town", "Screenshot"]], event: [["Museum visit", "11:00"], ["Flight to Lisbon", "07:10"], ["Dinner reservation", "20:00"]], contact: [["Hana Sato", "Host"], ["Taxi driver", "Mobile"]],
  transaction: [["Hotel deposit", "-€240.00"], ["Train tickets", "-€38.50"], ["Café", "-€4.20"]], workout: [["Morning run", "5.2 km"], ["Hike in Sintra", "11.8 km"]],
};
const KINDS10 = Object.keys(POOL) as Kind[];
const STATUS: Item["status"][] = ["To do", "Doing", "Done"];
function mixed(id: string, name: string, hue: number, big: BigView, n = 14): void {
  const h = [...name].reduce((a, c) => a + c.charCodeAt(0), 0);
  const items = Array.from({ length: n }, (_, i) => { const date = `2026-11-${String(1 + ((i * 5 + hue) % 27)).padStart(2, "0")}`; const kind = KINDS10[(i * 3 + h) % 10]!, p = POOL[kind]![(i + h) % POOL[kind]!.length]!; return mk(kind, `${id}-${i}`, p[0], p[1], { hue: (hue + i * 23) % 360, status: STATUS[(i + hue) % 3], due: kind === "task" ? day(date) : weekday(date), when: day(date), amount: kind === "transaction" ? p[1] : undefined, date, body: kind === "note" || kind === "file" ? `${p[1]}. A few more words to show the first lines of the page, then a second line of text.` : undefined, owner: ["Ada", "Ivo", "Hana", "Jun"][i % 4], n: kind === "chat" && i % 2 ? 2 : undefined, unread: kind === "mail" && i % 3 === 0 }); });
  def({ id, name, hue, library: true, small: "list", big, items });
}
mixed("lisbon", "Trip to Lisbon", 205, "list"); mixed("move", "House move", 28, "board"); mixed("taxes", "Taxes 2026", 150, "table"); mixed("q4", "Q4 planning", 280, "board"); mixed("recipes", "Recipes", 12, "gallery"); mixed("reading", "Reading list", 55, "list"); mixed("family", "Family archive", 330, "gallery", 20); mixed("bike", "Bike repair", 175, "list", 9);
// the nothing-selected sidebar: what was touched lately, across apps
def({ id: "recent-all", name: "Recent", hue: 205, small: "list", big: "list", items: pick("m-0", "c-lunch", "n-0", "t-0", "e-1", "fr-0", "m-1") });

/** The library, in the card's Collections tab. */
export const LIBRARY = ["lisbon", "recent-mail", "recent-notes", "tasks-week", "unread-chats", "move", "taxes", "q4", "recipes", "reading", "family", "bike"];
/** Each app's sidebar by default: an ordered list of collections. Actions (New note, Write) are fixed rows above them, not collections. */
export const DEFAULT_SIDEBARS: Record<string, string[]> = {
  chat: ["chat-pins", "groups", "people", "cals", "harnesses", "archived"], notes: ["notes-pinned", "recent-notes", "tags"], mail: ["mailboxes", "recent-mail", "labels"], files: ["f-fav", "f-loc", "f-recent"], photos: ["p-lib", "p-albums", "p-people"],
  calendar: ["cal-cals", "cal-up"], contacts: ["c-groups", "c-people"], tasks: ["t-smart", "tasks-week", "t-lists"], settings: ["settings-list"],
};
export const ACTIONS: Record<string, string> = { notes: "New note", mail: "Write", calendar: "New event", contacts: "New contact", tasks: "New task" };
/** The sidebar that belongs to no app, seeded: "Lisbon trip" with three sections. */
export const SEED_CUSTOM = { id: "cs-lisbon", name: "Lisbon trip", sections: ["lisbon", "tasks-week", "recent-notes"] };
/** The app that opens an item of a kind in place; kinds with none (transaction, workout) always open a window of their own. */
export const KIND_APP: Partial<Record<Kind, string>> = { chat: "chat", note: "notes", mail: "mail", task: "tasks", file: "files", photo: "photos", event: "calendar", contact: "contacts" };
