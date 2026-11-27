/**
 * Fixture data of the Notes app: invented notes written as blocks, and the collections its sidebar draws. Nothing here is real or saved.
 * A note is an `Item` of kind "note" (the shell's type, so every collection can hold one); its page is a `Doc` kept here by the item's id. A note the shell made up (a mixed collection's "Packing list") has no Doc: `docOf` writes a small one from its Item.
 * Text is written in a tiny markup so the fixture stays readable: `[[nt-id]]` links a note (drawn as a chip), `**bold**`, `_italic_`, `` `code` ``, `==selected==` (the selection state), `^` (the caret).
 */
import { ITEMS, def, mk, type Item } from "../../data/collections";

export type Run = string | { b: string } | { i: string } | { code: string } | { note: string } | { sel: string } | { caret: true } | { href: string; text: string };
export type Block =
  | { t: "h1" | "h2" | "h3"; runs: Run[] } | { t: "p"; runs: Run[] } | { t: "ul" | "ol"; items: Run[][] } | { t: "todo"; ids: string[] }
  | { t: "quote"; runs: Run[]; by?: string } | { t: "code"; lang: string; text: string } | { t: "table"; head: string[]; rows: string[][] }
  | { t: "image"; hue: number; caption: string } | { t: "hr" };
export interface Doc { id: string; folder: string; tags: string[]; /** Cover colour (a hue), or none. */ cover?: number; blocks: Block[]; edited: string; author: string; shared: { name: string; role: "Owner" | "Editor" | "Viewer" }[]; history: { who: string; when: string; what: string }[] }

/** Writes runs from the tiny markup. */
export function runs(s: string): Run[] {
  const out: Run[] = [], re = /\[\[(nt-[\w-]+)\]\]|\*\*(.+?)\*\*|_(.+?)_|`(.+?)`|==(.+?)==|\^|\[(.+?)\]\((.+?)\)/g;
  let at = 0, m: RegExpExecArray | null;
  while ((m = re.exec(s))) {
    if (m.index > at) out.push(s.slice(at, m.index));
    out.push(m[1] ? { note: m[1] } : m[2] ? { b: m[2] } : m[3] ? { i: m[3] } : m[4] ? { code: m[4] } : m[5] ? { sel: m[5] } : m[6] ? { href: m[7]!, text: m[6] } : { caret: true });
    at = re.lastIndex;
  }
  if (at < s.length) out.push(s.slice(at));
  return out;
}
const h1 = (s: string): Block => ({ t: "h1", runs: runs(s) }), h2 = (s: string): Block => ({ t: "h2", runs: runs(s) }), h3 = (s: string): Block => ({ t: "h3", runs: runs(s) }), p = (s: string): Block => ({ t: "p", runs: runs(s) });
const ul = (...s: string[]): Block => ({ t: "ul", items: s.map(runs) }), ol = (...s: string[]): Block => ({ t: "ol", items: s.map(runs) });
const todo = (...ids: string[]): Block => ({ t: "todo", ids }), quote = (s: string, by?: string): Block => ({ t: "quote", runs: runs(s), by });
const code = (lang: string, text: string): Block => ({ t: "code", lang, text }), table = (head: string[], ...rows: string[][]): Block => ({ t: "table", head, rows });
const image = (hue: number, caption: string): Block => ({ t: "image", hue, caption }), hr: Block = { t: "hr" };

export const DOCS: Record<string, Doc> = {};
const ME = "Ada Lindqvist", team = (...n: string[]) => [{ name: ME, role: "Owner" as const }, ...n.map((x, i) => ({ name: x, role: i ? ("Viewer" as const) : ("Editor" as const) }))];
interface Spec { id: string; title: string; folder: string; tags: string[]; hue: number; cover?: number; when: string; date: string; blocks: Block[]; by?: string[]; hist?: Doc["history"] }
const specs: Spec[] = [
  { id: "nt-lisbon", title: "Autumn trip to Lisbon", folder: "Travel", tags: ["travel", "lisbon"], hue: 205, cover: 205, when: "Today", date: "2026-11-20", by: ["Hana Sato", "Ivo Novak"], blocks: [
    p("Four days in the old town with Hana: **14 to 18 November** was the first idea, then we moved it a week. Flights are booked and the hotel is still open for a change. See the [[nt-packing]] list and the [[nt-museums]] plan."),
    h2("Before we go"),
    todo("tk-tickets", "tk-charger", "tk-tram", "tk-hotel"),
    p("Things to remember:"),
    ul("Check the tram pass covers the **funicular**", "Ask the hotel about a late check-in, we land at ==ten at night==^", "Bring cash for the small cafés, cards are not always taken"),
    h2("The days"),
    table(["Day", "Morning", "Evening", "Booked"], ["Sat", "Alfama on foot", "Fado at Tasca", "Yes"], ["Sun", "Tile museum", "Dinner by the river", "Tickets open"], ["Mon", "Sintra, by train", "Early night", "No"], ["Tue", "Market, then the airport", "", "Yes"]),
    h2("Photos"),
    image(205, "The view from the hotel terrace, 2019. We want the same one."),
    p("Convert the phone's photos before sharing the album:"),
    code("sh", "# HEIC to JPEG, keeping the dates\nfor f in *.heic; do\n  heif-convert -q 90 \"$f\" \"${f%.heic}.jpg\"\ndone\nexiftool -TagsFromFile %f.heic -all:all *.jpg"),
    quote("Lisbon is a city you climb before you understand it.", "a guidebook, borrowed"),
    hr,
    p("Next: write down the restaurants Hana suggested, and decide about Sintra. See also the [travel notes from 2019](https://example.org/lisbon-2019)."),
  ], hist: [{ who: "Hana Sato", when: "2 h ago", what: "Edited “The days”" }, { who: ME, when: "Yesterday", what: "Added 4 tasks" }, { who: "Ivo Novak", when: "Mon", what: "Commented on the hotel" }, { who: ME, when: "12 Nov", what: "Created" }] },
  { id: "nt-packing", title: "Packing list", folder: "Travel", tags: ["travel"], hue: 205, when: "Yesterday", date: "2026-11-19", blocks: [p("What goes in the small bag for [[nt-lisbon]]."), ul("Passport (renew first!)", "Charger and plug adapter", "Rain jacket", "Two books", "Walking shoes, already worn in")] },
  { id: "nt-museums", title: "Museums and what is closed when", folder: "Travel", tags: ["travel", "lisbon"], hue: 205, cover: 175, when: "Wed", date: "2026-11-18", blocks: [h2("Opening hours"), table(["Museum", "Open", "Closed"], ["Tiles", "10:00 to 18:00", "Monday"], ["River museum", "10:00 to 19:00", "Never"], ["Castle", "9:00 to 21:00", "Never"]), p("Back to [[nt-lisbon]].")] },
  { id: "nt-rewrite", title: "Calternal rewrite", folder: "Work", tags: ["work", "calternal"], hue: 280, when: "Mon", date: "2026-11-16", by: ["Ivo Novak"], blocks: [
    p("The plan for the three regions: web first, then the Mac app, then the phone. Everything is a plug-in that **describes** its interface; each platform draws the description natively."),
    h2("The seam"), code("ts", "export interface MockupApp {\n  id: string; name: string; icon: string;\n  collections: Coll[]; sidebar: string[];\n  page?: Component<PageProps>;\n}"),
    h2("Who does what"), ul("Ada: interface and the mockups", "Ivo: build and the host", "Hana: copy and the website"), p("Notes from the review are in [[nt-ada]]."),
  ] },
  { id: "nt-ada", title: "Meeting with Ada", folder: "Work", tags: ["work"], hue: 280, when: "Thu", date: "2026-11-19", blocks: [p("Decisions and who does what."), h3("Sidebar"), ul("Rows are one line", "Eight pixels between sections"), h3("Follow up"), todo("tk-spacing", "tk-icon"), p("Context in [[nt-rewrite]].")] },
  { id: "nt-colemak", title: "Colemak, week three", folder: "Journal", tags: ["journal"], hue: 150, when: "Sun", date: "2026-11-15", blocks: [p("Typing speed is back to **62 words a minute** after the second week. Home row first, then the top row."), ol("Ten minutes before work", "One real text a day", "No switching back, not even for passwords")] },
  { id: "nt-dal", title: "Dal makhani", folder: "Cooking", tags: ["recipes"], hue: 28, cover: 28, when: "Sat", date: "2026-11-14", blocks: [p("Soak the beans overnight. Eight hours on low, then cream and butter at the end."), h2("Ingredients"), ul("250 g whole black lentils", "50 g kidney beans", "2 tbsp butter", "100 ml cream"), h2("Method"), ol("Soak lentils and beans overnight, drain.", "Simmer with ginger and tomato for an hour.", "Add butter; keep it on low for another six hours.", "Finish with cream and salt.")] },
  { id: "nt-design", title: "The design of everyday things", folder: "Reading", tags: ["reading"], hue: 55, when: "Fri", date: "2026-11-13", blocks: [quote("A door that needs a sign has been designed wrongly.", "Don Norman"), p("Affordances and signifiers: what a thing allows, and what tells you it does."), ul("Mapping: controls laid out like what they control", "Feedback: say what happened", "Constraints: make the wrong thing hard")] },
  { id: "nt-today", title: "Friday 20 November", folder: "Journal", tags: ["journal"], hue: 150, when: "Today", date: "2026-11-20", blocks: [p("Slept badly. Cold morning; first snow on the hills."), todo("tk-passport", "tk-invoice"), p("Lunch with Hana. Talked about the trip and the flat.")] },
  { id: "nt-books", title: "Reading list 2026", folder: "Reading", tags: ["reading"], hue: 55, when: "2w", date: "2026-11-06", blocks: [ul("The Design of Everyday Things", "Bird by Bird", "A Pattern Language, the second half"), p("Notes on the first in [[nt-design]].")] },
  { id: "nt-empty", title: "Untitled", folder: "Journal", tags: [], hue: 205, when: "Now", date: "2026-11-20", blocks: [] },
];
const LONG = [
  ["Why this note is long", "A long note is a good test for a page: the title scrolls away, the outline in the side panel takes over, and a reader needs to know where they are. This one has eight sections, each a few paragraphs, and nothing in it matters."],
  ["The first week", "Monday was slow. The mail piled up and the build broke twice before lunch, once for a reason nobody could explain and once because someone had pulled a branch that had not been merged. By the afternoon the team had agreed to stop touching the build until Wednesday."],
  ["What changed in the middle", "On Wednesday the plan changed. Instead of three regions at once, the first release would be the web app alone, with the Mac app starting as soon as the description of the interface was stable. Nobody argued; it was the smaller plan."],
  ["Typography", "A line of text is comfortable at about sixty-five characters. Shorter than forty and the eye jumps too often; longer than ninety and it loses its place on the way back. Everything else in a page follows from that measure: the size of the type, the space between lines, the width of the column."],
  ["Spacing", "Space is the cheapest way to say that two things belong together. A heading sits closer to the paragraph it introduces than to the one before it; a list has the same gap between its items as between lines of a paragraph, plus a little."],
  ["Colour and contrast", "Secondary text must stay legible over any background. That means checking it, not guessing: four and a half to one is the line, and it is surprisingly easy to fall under it with a soft grey on a light wallpaper."],
  ["Motion", "Motion should say what happened: where a thing came from, where it went. If it only decorates, remove it. Everything interruptible, everything instant for people who asked for less."],
  ["What is next", "Write down what is left, then do the smallest piece of it. The rest will look different once that one is done."],
] as const;
const longBlocks: Block[] = [p("^" + LONG[0][1]), ...LONG.slice(1).flatMap(([t, s], i): Block[] => [h2(t), p(s), p(["A second paragraph for the same section, so the page keeps going. It says a thought again in other words, which is what long notes do.", "Nothing here needs deciding. It is a stretch of ordinary text, long enough to wrap onto a second line and a third, so the measure and the rhythm can be judged.", "Read it once for the sense and once for the shape: where the eye rests, where a line breaks badly, whether the gap before a heading is enough."][i % 3]!), ...(i === 2 ? [ul("The smaller plan", "The same team", "A date on the calendar")] : []), ...(i === 4 ? [table(["Element", "Gap"], ["Heading to paragraph", "var(--s-3)"], ["Paragraph to paragraph", "var(--s-5)"], ["Section to section", "var(--s-8)"])] : [])])];
specs.push({ id: "nt-long", title: "A long read on type, space and motion", folder: "Reading", tags: ["reading", "design"], hue: 55, cover: 330, when: "Wed", date: "2026-11-18", blocks: longBlocks });

/** First readable text of a block list, for a thumbnail or a row's quiet line. */
export const plain = (rs: Run[]): string => rs.map((r) => (typeof r === "string" ? r : "b" in r ? r.b : "i" in r ? r.i : "code" in r ? r.code : "sel" in r ? r.sel : "text" in r ? r.text : "note" in r ? (ITEMS[r.note]?.title ?? "") : "")).join("");
export const firstLines = (blocks: Block[]): string => blocks.map((b) => (b.t === "p" || b.t === "quote" ? plain(b.runs) : b.t === "ul" || b.t === "ol" ? b.items.map(plain).join(" · ") : "")).filter(Boolean).slice(0, 2).join(" ");

export const notes: Item[] = specs.map((s) => {
  DOCS[s.id] = { id: s.id, folder: s.folder, tags: s.tags, cover: s.cover, blocks: s.blocks, edited: s.when, author: ME, shared: team(...(s.by ?? [])), history: s.hist ?? [{ who: ME, when: s.when, what: "Edited" }, { who: ME, when: "12 Nov", what: "Created" }] };
  const body = firstLines(s.blocks);
  return mk("note", s.id, s.title, body.slice(0, 90) || "Empty", { when: s.when, body, hue: s.hue, date: s.date });
});
const byId = (...ids: string[]) => ids.map((i) => ITEMS[i]!);

/** The page of a note: its Doc, or one written from the Item when the shell made the note up. */
export function docOf(item: Item): Doc {
  return DOCS[item.id] ?? { id: item.id, folder: "Notes", tags: [], blocks: [p(item.body || item.sub || ""), p("This note was made up by a collection of the shell; its page is a short stand-in.")], edited: item.when ?? "Today", author: ME, shared: team(), history: [{ who: ME, when: item.when ?? "Today", what: "Edited" }] };
}
/** Notes whose text links to `id`, and the tasks that came out of it. */
export const linksTo = (id: string): Item[] => notes.filter((n) => n.id !== id && DOCS[n.id]!.blocks.some((b) => (b.t === "p" || b.t === "quote" ? b.runs : b.t === "ul" || b.t === "ol" ? b.items.flat() : []).some((r) => typeof r === "object" && "note" in r && r.note === id)));
export const linksFrom = (id: string): Item[] => { const out = new Set<string>(); for (const b of DOCS[id]?.blocks ?? []) for (const r of b.t === "p" || b.t === "quote" ? b.runs : b.t === "ul" || b.t === "ol" ? b.items.flat() : []) if (typeof r === "object" && "note" in r && r.note !== id) out.add(r.note); return [...out].map((i) => ITEMS[i]!).filter(Boolean); };
export const headings = (blocks: Block[]) => blocks.filter((b): b is Extract<Block, { t: "h1" | "h2" | "h3" }> => b.t === "h1" || b.t === "h2" || b.t === "h3").map((b) => ({ level: b.t === "h3" ? 3 : 2, text: plain(b.runs) }));
export const wordCount = (id: string) => 40 + (DOCS[id]?.blocks.length ?? 2) * 34;

export const FOLDERS = ["Travel", "Cooking", "Work", "Journal", "Reading", "Archive"];
export const TAGS = [["travel", 205], ["recipes", 28], ["work", 280], ["reading", 55], ["journal", 150]] as const;
const inFolder = (f: string) => notes.filter((n) => DOCS[n.id]!.folder === f).length;
/** A folder or tag row opens a collection of its notes (the shell's `opens`). */
const opens = (label: Item, id: string, name: string, hue: number, items: Item[]): Item => { def({ id, name, hue, small: "list", big: "gallery", items }); label.opens = id; return label; };
export const collections = [
  def({ id: "notes-pinned", name: "Pinned", hue: 55, pin: true, small: "strip", big: "gallery", items: byId("nt-lisbon", "nt-rewrite", "nt-dal") }),
  def({ id: "recent-notes", name: "Recent", hue: 55, library: true, small: "list", big: "gallery", items: byId("nt-today", "nt-lisbon", "nt-packing", "nt-museums", "nt-ada", "nt-long", "nt-colemak", "nt-dal", "nt-design", "nt-books", "nt-rewrite", "nt-empty") }),
  def({ id: "nt-folders", name: "Folders", hue: 205, small: "list", big: "list", items: FOLDERS.map((f) => opens(mk("label", `ntf-${f.toLowerCase()}`, f, "", { icon: "folder", count: inFolder(f) || undefined }), `ntc-${f.toLowerCase()}`, f, 205, notes.filter((n) => DOCS[n.id]!.folder === f))) }),
  def({ id: "tags", name: "Tags", hue: 150, small: "list", big: "list", items: TAGS.map(([t, h]) => opens(mk("label", `ntg-${t}`, `#${t}`, "", { hue: h, count: notes.filter((n) => DOCS[n.id]!.tags.includes(t)).length }), `ntt-${t}`, `#${t}`, h, notes.filter((n) => DOCS[n.id]!.tags.includes(t)))) }),
];
