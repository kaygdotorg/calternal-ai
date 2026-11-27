/**
 * Fixture data of the Photos mockup: about 190 invented photographs over seven weeks (each only a generated scene, see `scene.ts`), the albums and library lists made from them, and the details the side panel shows (date, place, camera, people).
 * Items are the shell's `Item`s (kind `photo`); an album or a library list is a `photo` item with `folder: true`, which opens in place as a page of the Photos window. Everything else about a photo is in `META`, keyed by the item id. Nothing here is real.
 */
import { day, dayLong, stamp } from "../../lib/when";
import { def, mk, ITEMS, type Item } from "../../data/collections";
import { rng, type SceneKind, type SceneSpec } from "./scene";

export interface PhotoMeta {
  id: string; /** ISO date of the shot. */ date: string; time: string; place: string; city: string; people: string[]; fav: boolean; albums: string[];
  camera: string; lens: string; exposure: string; bytes: number; w: number; h: number; /** Seconds, for a video. */ video?: number; spec: SceneSpec; added: string;
}
export const TODAY = "2026-11-20";
const DAY = 86400000;
const iso = (back: number) => new Date(Date.parse(TODAY) - back * DAY).toISOString().slice(0, 10);
/** The day heading above a group of the timeline: Today, Yesterday, the weekday inside a week, then the full date. */
export function dayLabel(d: string): string { const back = Math.round((Date.parse(TODAY) - Date.parse(d)) / DAY); return back === 0 ? "Today" : back === 1 ? "Yesterday" : dayLong(d) + (back > 330 ? ` ${d.slice(0, 4)}` : ""); }
export const longDate = (d: string, t?: string) => stamp(d, t);
export const shortDate = (d: string) => day(d);
export const bytesText = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);

interface Plan { back: number; n: number; place: string; city: string; kinds: SceneKind[]; hue: number; tags: string[]; people?: string[]; times?: ("day" | "dusk" | "night")[] }
const PLAN: Plan[] = [
  { back: 0, n: 4, place: "Kitchen", city: "Home", kinds: ["table", "blobs", "hills"], hue: 40, tags: [] },
  { back: 1, n: 9, place: "Parque da Cidade", city: "Porto", kinds: ["hills", "blobs", "dunes", "coast"], hue: 140, tags: ["autumn"], people: ["Ada", "Ben"] },
  { back: 3, n: 6, place: "Rua do Almada", city: "Porto", kinds: ["city", "table", "coast", "hills"], hue: 20, tags: [] },
  { back: 5, n: 12, place: "Serra da Arrábida", city: "Setúbal", kinds: ["hills", "coast", "dunes", "blobs", "table"], hue: 175, tags: ["autumn", "family"], people: ["Hana", "Lena", "Ada"] },
  { back: 6, n: 5, place: "Home", city: "Porto", kinds: ["table", "blobs", "hills"], hue: 50, tags: ["family"], people: ["Lena"] },
  { back: 7, n: 14, place: "Alfama", city: "Lisbon", kinds: ["city", "coast", "hills"], hue: 205, tags: ["lisbon"], people: ["Ada", "Ben"] },
  { back: 8, n: 11, place: "Cabo da Roca", city: "Sintra", kinds: ["coast", "hills", "dunes"], hue: 215, tags: ["lisbon"], people: ["Ada"], times: ["day", "dusk"] },
  { back: 9, n: 13, place: "Belém", city: "Lisbon", kinds: ["city", "coast", "table"], hue: 190, tags: ["lisbon"], people: ["Ben", "Ada"] },
  { back: 10, n: 8, place: "Bairro Alto", city: "Lisbon", kinds: ["city", "table"], hue: 25, tags: ["lisbon"], times: ["dusk", "night"] },
  { back: 12, n: 7, place: "Campo de Ourique", city: "Lisbon", kinds: ["city", "blobs"], hue: 340, tags: ["lisbon"] },
  { back: 16, n: 10, place: "Home", city: "Porto", kinds: ["blobs", "table"], hue: 30, tags: ["moving"], people: ["Ivo", "Ben"] },
  { back: 17, n: 13, place: "Rua das Flores 12", city: "Porto", kinds: ["table", "city", "blobs"], hue: 35, tags: ["moving"], people: ["Ivo", "Jun", "Ben"] },
  { back: 21, n: 9, place: "Douro riverside", city: "Porto", kinds: ["coast", "hills"], hue: 195, tags: ["bike", "autumn"], people: ["Ada"], times: ["day", "dusk"] },
  { back: 24, n: 12, place: "Ecovia do Tâmega", city: "Amarante", kinds: ["hills", "dunes", "coast"], hue: 150, tags: ["bike"], people: ["Ben", "Ada"] },
  { back: 28, n: 8, place: "Praia de Matosinhos", city: "Matosinhos", kinds: ["coast", "dunes"], hue: 220, tags: ["family"], people: ["Lena", "Hana"] },
  { back: 33, n: 10, place: "Gaia", city: "Porto", kinds: ["city", "coast", "blobs"], hue: 15, tags: ["family"], people: ["Hana", "Ada"] },
  { back: 41, n: 8, place: "Home", city: "Porto", kinds: ["table", "blobs"], hue: 60, tags: [], people: ["Lena"] },
  { back: 47, n: 9, place: "Serra do Gerês", city: "Gerês", kinds: ["hills", "dunes"], hue: 130, tags: ["autumn", "bike"], people: ["Ben", "Ada"], times: ["day", "dusk"] },
];
const CAM = [["Lumio 12 Pro", "Main camera, 24 mm f/1.8"], ["Lumio 12 Pro", "Ultra wide, 13 mm f/2.2"], ["Lumio 12 Pro", "Telephoto, 77 mm f/2.8"], ["Fable X100", "35 mm f/2"]] as const;

/** Every photo, newest first. */
export const PHOTOS: PhotoMeta[] = [];
const r = rng(4121);
let n = 0;
for (const p of PLAN) {
  for (let i = 0; i < p.n; i++, n++) {
    const kind = p.kinds[(i + Math.floor(r() * 2)) % p.kinds.length]!, time = (p.times ?? ["day"])[Math.floor(r() * (p.times?.length ?? 1))]!, cam = CAM[Math.floor(r() * CAM.length)]!;
    const hh = time === "night" ? 21 + Math.floor(r() * 2) : time === "dusk" ? 17 + Math.floor(r() * 2) : 9 + Math.floor(r() * 8), mm = Math.floor(r() * 60);
    const portrait = r() < 0.18, video = r() < 0.07 ? 6 + Math.floor(r() * 80) : undefined;
    const id = `ph-${n}`;
    PHOTOS.push({
      id, date: iso(p.back), time: `${String(hh).padStart(2, "0")}:${String(mm).padStart(2, "0")}`, place: p.place, city: p.city, people: (p.people ?? []).filter(() => r() < 0.7), fav: r() < 0.09, albums: p.tags,
      camera: cam[0], lens: cam[1], exposure: `f/${[1.8, 2.2, 2.8, 4][Math.floor(r() * 4)]}  1/${[60, 125, 250, 500, 1000][Math.floor(r() * 5)]} s  ISO ${[50, 64, 100, 200, 400][Math.floor(r() * 5)]}`,
      bytes: Math.round((video ? 12e6 + video * 4e6 : 1.6e6 + r() * 3.4e6)), w: portrait ? 3024 : 4032, h: portrait ? 4032 : 3024, video, spec: { kind, hue: (p.hue + Math.floor(r() * 90) - 45 + 360) % 360, seed: n + 11, time }, added: iso(Math.max(0, p.back - Math.floor(r() * 2))),
    });
  }
}
/** The photo items (the shell's `Item`), newest first, and their details by id. */
export const META: Record<string, PhotoMeta> = {};
export const PHOTO_ITEMS: Item[] = PHOTOS.map((m) => { META[m.id] = m; return mk("photo", m.id, `IMG_${String(2400 + Number(m.id.slice(3))).padStart(4, "0")}${m.video ? ".MOV" : ""}`, m.place, { when: m.time, date: m.date, hue: m.spec.hue }); });
export const metaOf = (id: string): PhotoMeta | undefined => META[id];

/** The albums: a `photo` item with `folder: true`; its photos are those carrying its tag. */
export interface AlbumDef { id: string; tag: string; title: string; hue: number; sharedWith: string[]; note: string }
export const ALBUMS: AlbumDef[] = [
  { id: "al-lisbon", tag: "lisbon", title: "Lisbon", hue: 205, sharedWith: ["Ben Okafor", "Hana Sato"], note: "Five days, one long walk uphill." },
  { id: "al-moving", tag: "moving", title: "Moving day", hue: 28, sharedWith: ["Ivo Novak"], note: "" },
  { id: "al-bike", tag: "bike", title: "Bike", hue: 150, sharedWith: [], note: "Weekends on the river path and the Tâmega." },
  { id: "al-family", tag: "family", title: "Family", hue: 330, sharedWith: ["Lena Lindqvist", "Hana Sato", "Ada Lindqvist"], note: "" },
  { id: "al-autumn", tag: "autumn", title: "Autumn walks", hue: 55, sharedWith: [], note: "" },
];
export const albumPhotos = (a: AlbumDef): PhotoMeta[] => PHOTOS.filter((p) => p.albums.includes(a.tag));
const cover = (a: AlbumDef) => albumPhotos(a)[0]!;
export const ALBUM_ITEMS = ALBUMS.map((a) => mk("photo", a.id, a.title, `${albumPhotos(a).length} photos`, { folder: true, hue: cover(a).spec.hue, count: albumPhotos(a).length }));
export const albumOf = (id: string) => ALBUMS.find((a) => a.id === id);

/** The library's lists, a `photo` item with `folder: true` and an icon. */
export interface ListDef { id: string; title: string; icon: string; pick: (p: PhotoMeta) => boolean }
export const LISTS: ListDef[] = [
  { id: "pl-all", title: "All photos", icon: "image", pick: () => true },
  { id: "pl-fav", title: "Favourites", icon: "spark", pick: (p) => p.fav },
  { id: "pl-video", title: "Videos", icon: "play", pick: (p) => !!p.video },
  { id: "pl-recent", title: "Recently added", icon: "clock", pick: (p) => Date.parse(TODAY) - Date.parse(p.added) <= 14 * DAY },
];
export const listOf = (id: string) => LISTS.find((l) => l.id === id);
export const LIST_ITEMS = LISTS.map((l) => mk("photo", l.id, l.title, "", { folder: true, icon: l.icon, count: PHOTOS.filter(l.pick).length }));

/** Photos of a day, in order, grouped: the timeline's sections. */
export function byDay(list: PhotoMeta[]): { date: string; photos: PhotoMeta[] }[] {
  const out: { date: string; photos: PhotoMeta[] }[] = [];
  for (const p of list) { const g = out[out.length - 1]; if (g && g.date === p.date) g.photos.push(p); else out.push({ date: p.date, photos: [p] }); }
  return out;
}

// Collections: Library (lists), Recently added (a strip of the newest), Albums (a strip of cover tiles).
def({ id: "p-lib", name: "Library", hue: 330, small: "list", big: "list", items: LIST_ITEMS.slice(0, 3) });
def({ id: "p-recent", name: "Recently added", hue: 330, library: true, small: "strip", big: "gallery", items: PHOTO_ITEMS.slice(0, 9) });
def({ id: "p-albums", name: "Albums", hue: 330, library: true, small: "strip", big: "gallery", items: ALBUM_ITEMS });
def({ id: "p-favs", name: "Favourite photos", hue: 330, library: true, small: "strip", big: "gallery", items: PHOTO_ITEMS.filter((i) => META[i.id]!.fav).slice(0, 10) });
void ITEMS;
