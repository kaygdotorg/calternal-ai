/**
 * Fixture data of the Files mockup: a small, believable file tree (a curated Home of Documents, Photos, Notes and Shared, as the Files grill decided) with about two hundred files of every family, one long folder, one empty folder, and the shared and favourite lists.
 * Items are the shell's `Item`s (kind `file`, `folder: true` for a folder); everything else about a file (size, dates, parent, sharing) is in `META`, keyed by the item id. Invented names and content; nothing here is real.
 */
import { day, stamp, time } from "../../lib/when";
import { def, mk, pick, type Item } from "../../data/collections";
import { bytesText, TODAY } from "../photos/data";
import { rng, type SceneSpec } from "../photos/scene";
import { BODY } from "./content";
import { typeOf, type FileType } from "./types";

export interface Share { /** People who can open it. */ with: string[]; /** An open link and what it allows; `expires` is an ISO date. */ link?: { can: "view" | "edit"; expires?: string; password?: boolean } }
export interface FileMeta {
  id: string; name: string; parent: string | null; folder: boolean; bytes: number; /** ISO date and time of the last change. */ modified: string; time: string; type: FileType; by: string;
  share?: Share; /** For a file shared with me: its owner. */ owner?: string; fav?: boolean; keep?: boolean; scene?: SceneSpec; pages?: number; versions: number;
}
export const META: Record<string, FileMeta> = {};
export const CHILDREN: Record<string, string[]> = {};
const DAY = 86400000;
const iso = (back: number) => new Date(Date.parse(TODAY) - back * DAY).toISOString().slice(0, 10);
/** A modified time as a person reads it in a list (lib/when.ts): a time today, else the shared list day. */
export function whenText(d: string, t: string): string { return d === TODAY ? time(t) : day(d); }
export const fullWhen = (m: FileMeta) => stamp(m.modified, m.time);

const r = rng(77);
const sizeOf = (ext: string): number => ({ pdf: 4e5 + r() * 1.6e6, docx: 5e4 + r() * 4e5, md: 2e3 + r() * 3e4, txt: 1e3 + r() * 9e3, xlsx: 3e4 + r() * 2e5, csv: 4e3 + r() * 6e4, pptx: 2e6 + r() * 9e6, png: 3e5 + r() * 2.4e6, jpg: 8e5 + r() * 3.2e6, heic: 9e5 + r() * 2.4e6, mov: 3e7 + r() * 2e8, mp4: 2e7 + r() * 1e8, m4a: 2e6 + r() * 2e7, mp3: 3e6 + r() * 8e6, zip: 4e6 + r() * 5e7, ts: 1e3 + r() * 2e4, json: 1e3 + r() * 4e4 })[ext] ?? 1e4;
let n = 0;
const reg = (m: FileMeta): FileMeta => { META[m.id] = m; if (m.parent) (CHILDREN[m.parent] ??= []).push(m.id); return m; };
const hhmm = () => `${String(8 + Math.floor(r() * 11)).padStart(2, "0")}:${String(Math.floor(r() * 60)).padStart(2, "0")}`;
/** A folder in the tree. */
function dir(id: string, parent: string | null, name: string, back: number, x: Partial<FileMeta> = {}): string {
  reg({ id, name, parent, folder: true, bytes: 0, modified: iso(back), time: hhmm(), type: typeOf(name, true), by: "You", versions: 0, ...x }); CHILDREN[id] ??= []; return id;
}
/** A file in a folder; its size is invented from its type unless given. */
function file(parent: string, name: string, back: number, x: Partial<FileMeta> & { size?: number; id?: string } = {}): string {
  const type = typeOf(name), id = x.id ?? `fi-${n++}`, { size, ...rest } = x;
  const scene: SceneSpec | undefined = type.family === "image" ? { kind: (["hills", "coast", "city", "dunes", "table", "blobs"] as const)[Math.floor(r() * 6)]!, hue: Math.floor(r() * 360), seed: 300 + n, time: "day" } : undefined;
  reg({ id, name, parent, folder: false, bytes: Math.round(size ?? sizeOf(name.slice(name.lastIndexOf(".") + 1))), modified: iso(back), time: hhmm(), type, by: "You", scene, versions: 1 + Math.floor(r() * 4), pages: type.family === "pdf" ? 1 + Math.floor(r() * 5) : undefined, ...rest }); return id;
}

const HOME = dir("fo-home", null, "Home", 0);
const DOCS = dir("fo-docs", HOME, "Documents", 0, { fav: true }), PICS = dir("fo-pics", HOME, "Photos", 0, { fav: true }), NOTES = dir("fo-notes", HOME, "Notes", 1), SHARED = dir("fo-shared", HOME, "Shared", 2, { share: { with: ["Ben Okafor", "Hana Sato", "Ivo Novak"] } });
// Documents
const TAX = dir("fo-tax", DOCS, "Taxes 2026", 3, { fav: true, share: { with: ["Dev Patel"] } }), TRIP = dir("fo-trip", DOCS, "Lisbon trip", 9, { fav: true, share: { with: ["Ben Okafor", "Hana Sato"], link: { can: "view", expires: "2026-11-30" } } }), HOUSE = dir("fo-house", DOCS, "House move", 16), WORK = dir("fo-work", DOCS, "Work", 2), SCANS = dir("fo-scans", DOCS, "Scans", 40);
file(DOCS, "Lease 2026.pdf", 1, { id: "fr-0", size: 412_000, pages: 6, fav: true, share: { with: ["Femi Hansen"] } }); file(DOCS, "Budget 2026.xlsx", 4, { id: "fr-1", size: 88_000, keep: true }); file(DOCS, "Passport scan.pdf", 70, { size: 1_240_000, pages: 2, keep: true });
file(DOCS, "Invoice 0412.pdf", 6, { size: 96_000, pages: 1 }); file(DOCS, "Insurance policy.pdf", 120, { size: 2_100_000, pages: 5 }); file(DOCS, "Contract draft.docx", 2, { size: 64_000, share: { with: ["Dev Patel"], link: { can: "edit", password: true } } }); file(DOCS, "Résumé.docx", 200, { size: 48_000 });
// Taxes: a long folder of receipts
file(TAX, "Form 1040 draft.pdf", 3, { size: 540_000, pages: 4 }); file(TAX, "Income summary.xlsx", 3, { size: 41_000 }); file(TAX, "Deductions.csv", 5, { size: 9_800 }); file(TAX, "Accountant letter.docx", 8, { size: 38_000 });
const REC = dir("fo-rec", TAX, "Receipts", 3);
const SHOPS = ["Café Linha", "Mercearia Sol", "Papelaria Norte", "Farmácia Central", "Livraria Lello", "Fnac", "Continente", "Padaria Real", "Ferragens Silva", "Taxi Porto", "CP Comboios", "Metro Lisboa"];
for (let i = 0; i < 52; i++) file(REC, `${SHOPS[i % SHOPS.length]} ${String(((i * 7) % 28) + 1).padStart(2, "0")}-${String((i % 11) + 1).padStart(2, "0")}.${i % 5 === 0 ? "jpg" : "pdf"}`, 5 + i * 5, { size: 60_000 + r() * 700_000, pages: 1 });
// Lisbon trip
file(TRIP, "Itinerary.pdf", 12, { id: "fi-itin", size: 188_000, pages: 3, keep: true }); file(TRIP, "Flight tickets.pdf", 20, { size: 312_000, pages: 2, keep: true }); file(TRIP, "Hotel booking.pdf", 18, { size: 244_000, pages: 2 }); file(TRIP, "Packing list.md", 11, { size: 1_800 }); file(TRIP, "Timetable.txt", 13, { size: 4_100 });
file(TRIP, "Tram 28.jpg", 9, { size: 3_200_000 }); file(TRIP, "Sunset at Cabo da Roca.jpg", 8, { id: "fi-sunset", size: 2_700_000, fav: true }); file(TRIP, "Map of Alfama.png", 14, { size: 1_900_000 });
// House move
file(HOUSE, "Floor plan.png", 20, { size: 1_400_000 }); file(HOUSE, "Keys handover.pdf", 16, { size: 120_000, pages: 1 }); file(HOUSE, "Utility transfer.docx", 17, { size: 52_000 }); file(HOUSE, "Moving checklist.md", 22, { size: 2_400 }); file(HOUSE, "Boxes inventory.xlsx", 21, { size: 33_000 });
// Work
file(WORK, "Q4 planning.pptx", 2, { id: "fr-2", size: 6_800_000, share: { with: ["Ivo Novak", "Jun Park"] } }); file(WORK, "Roadmap.pptx", 9, { size: 4_100_000 }); file(WORK, "Standup notes.md", 1, { id: "fr-3", size: 5_600 }); file(WORK, "Interviews.zip", 30, { id: "fi-zip", size: 48_000_000 }); file(WORK, "Demo recording.mov", 12, { id: "fi-demo", size: 184_000_000 });
file(WORK, "Intro call.m4a", 6, { id: "fi-intro", size: 9_500_000 }); file(WORK, "API sketch.ts", 3, { id: "fi-code", size: 4_200 }); file(WORK, "Sample response.json", 3, { size: 12_400 });
// Photos folder
dir("fo-p26", PICS, "2026", 1); dir("fo-p25", PICS, "2025", 340); dir("fo-shots", PICS, "Screenshots", 5);
for (const [name, back] of [["IMG_2406.jpg", 1], ["IMG_2407.jpg", 1], ["IMG_2398.heic", 5], ["IMG_2391.jpg", 7], ["IMG_2377.jpg", 8], ["IMG_2352.jpg", 9]] as const) file(PICS, name, back, { size: 2_000_000 + r() * 2_000_000 });
// Notes
file(NOTES, "Autumn trip.md", 0, { id: "fr-4", size: 3_100 }); file(NOTES, "Calternal rewrite plan.md", 2, { id: "fi-plan", size: 41_000, keep: true }); file(NOTES, "Colemak, week three.md", 6, { size: 2_200 }); file(NOTES, "Dal makhani.md", 40, { size: 1_600 }); file(NOTES, "Reading list.md", 14, { size: 3_800 });
// Shared (with me, and by me)
file(SHARED, "Family finances.xlsx", 2, { size: 120_000, owner: "Ada Lindqvist", share: { with: ["Ada Lindqvist", "Ben Okafor"] } }); const BOOK = dir("fo-book", SHARED, "Book club 2026", 4, { owner: "Hana Sato", share: { with: ["Hana Sato", "Eva Sato", "Jun Park"] } });
file(BOOK, "November pick.pdf", 4, { size: 1_900_000, pages: 5, owner: "Hana Sato" }); file(BOOK, "Discussion questions.docx", 10, { size: 31_000, owner: "Hana Sato" }); file(SHARED, "Garden plan.png", 12, { size: 2_300_000, owner: "Femi Hansen", share: { with: ["Femi Hansen"] } }); file(SHARED, "Trip photos (Hana).zip", 6, { size: 212_000_000, owner: "Hana Sato" });

export const metaOf = (id: string): FileMeta | undefined => META[id];
export const kids = (id: string): FileMeta[] => (CHILDREN[id] ?? []).map((c) => META[c]!);
/** The folder's size: the sum of what it holds, folders included. */
export function sizeOfFolder(id: string): number { return kids(id).reduce((s, k) => s + (k.folder ? sizeOfFolder(k.id) : k.bytes), 0); }
export const countIn = (id: string): number => kids(id).length;
export const sizeText = (m: FileMeta): string => (m.folder ? `${countIn(m.id)} item${countIn(m.id) === 1 ? "" : "s"}` : bytesText(m.bytes));
/** The folders from Home down to this item, for the breadcrumb. */
export function trail(id: string): FileMeta[] { const out: FileMeta[] = []; for (let m: FileMeta | undefined = META[id]; m; m = m.parent ? META[m.parent] : undefined) out.unshift(m); return out; }

/** The shell's items, built once from the tree. */
export const ITEM: Record<string, Item> = {};
for (const m of Object.values(META)) {
  const body = m.type.family === "pdf" || m.type.family === "doc" || m.type.family === "text" ? (BODY[m.name] ?? "") : "";
  ITEM[m.id] = mk("file", m.id, m.name, sizeText(m), { when: whenText(m.modified, m.time), folder: m.folder || undefined, hue: m.type.hue, body: body || undefined, date: m.modified });
}
export const item = (id: string): Item => ITEM[id]!;

// Collections: Recent (a table in big), Shared with me, Favourites, the top folders.
const byDate = (a: FileMeta, b: FileMeta) => (a.modified === b.modified ? b.time.localeCompare(a.time) : b.modified.localeCompare(a.modified));
const recent = Object.values(META).filter((m) => !m.folder && !m.owner).sort(byDate).slice(0, 8);
def({ id: "f-recent", name: "Recent", hue: 205, small: "list", big: "table", library: true, items: recent.map((m) => item(m.id)) });
def({ id: "f-shared", name: "Shared with me", hue: 175, small: "list", big: "list", library: true, items: Object.values(META).filter((m) => m.owner && m.parent === SHARED).map((m) => item(m.id)) });
def({ id: "f-fav", name: "Favourites", hue: 205, small: "list", big: "list", items: Object.values(META).filter((m) => m.fav).map((m) => item(m.id)) });
def({ id: "f-top", name: "Folders", hue: 235, small: "list", big: "list", items: [DOCS, PICS, NOTES, SHARED].map((id) => item(id)) });
void pick;
