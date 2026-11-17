/**
 * Where wallpapers come from: a small built-in set, the person's own uploads, and Unsplash search, all behind one adapter. The server will store
 * what is chosen (pages never load from Unsplash or any other site while the app is in use: the server downloads a picked photo and serves it
 * from its own storage). `FixtureLibrary` is the in-browser stand-in: the built-ins are painted from palettes here, uploads live in IndexedDB,
 * and "Unsplash" results are procedurally generated pictures with fake credits. Fixture-only; nothing here ships with the real client.
 */

/** One picture the person can use. `url` is something an `<img>` can load (a same-origin file or a blob URL). A credit is shown wherever the picture is chosen. */
export interface Wallpaper {
  id: string;
  name: string;
  source: "builtin" | "upload" | "unsplash";
  url: string;
  /** For a generated look or a photograph, the dark scheme's rendition (the same recipe with lightness remapped); `url` is the light one. */
  urlDark?: string;
  /** A small version for the picker (empty for a generated look until `thumb()` has rendered it). */
  thumb: string;
  /** A generated look: its recipe id. Its pictures are rendered on demand (`resolve`, `thumb`), not when the library loads. */
  recipe?: string;
  /** For an Unsplash photo: who took it and where it is, as their licence asks. */
  credit?: { author: string; link: string };
  /** The stored size, for pictures kept in the library (uploads, saved photos, pasted links). */
  bytes?: number;
}

/** One search hit, not yet stored. */
export interface StockResult {
  id: string;
  description: string;
  thumb: string;
  credit: { author: string; link: string };
}

/** The wallpaper library the settings sheet talks to. */
export interface WallpaperLibrary {
  /** Built-ins first, then the person's uploads and saved stock photos. */
  list(): Promise<Wallpaper[]>;
  /** The image to show for a wallpaper in a scheme (renders a generated look the first time, then remembers it). */
  resolve?(w: Wallpaper, scheme: "light" | "dark"): Promise<string>;
  /** The picker thumbnail of a wallpaper (renders a generated look the first time). */
  thumbOf?(w: Wallpaper): Promise<string>;
  /** Stores an image file the person chose. */
  upload(file: File): Promise<Wallpaper>;
  /** Stores the picture a pasted link points to (the real server downloads it; the fixture can only when the picture's site allows it). */
  fromLink?(url: string): Promise<Wallpaper>;
  /** Removes an upload or saved photo (built-ins cannot be removed). */
  remove(id: string): Promise<void>;
  /** Searches Unsplash. Needs the server's access key; without it the real implementation returns an empty list and says why. */
  search(query: string): Promise<StockResult[]>;
  /** The server downloads the photo and stores it; returns it as a library wallpaper. */
  save(result: StockResult): Promise<Wallpaper>;
}

interface Blob2 { x: number; y: number; r: number; c: string }
interface Def { id: string; name: string; base: [string, string]; blobs: Blob2[] }

/** The default photographs are fetched at build time (scripts/wallpapers.mjs, list in wallpapers.json) into `public/wallpapers` and are never committed. */
const BASE = import.meta.env.BASE_URL;

/** A simple string hash, for deterministic fake stock pictures. */
const hash = (s: string): number => [...s].reduce((a, c) => (Math.imul(a, 31) + c.charCodeAt(0)) >>> 0, 7);

function paint(def: Def, w: number, h: number): Promise<string> {
  const cv = document.createElement("canvas");
  cv.width = w;
  cv.height = h;
  const ctx = cv.getContext("2d")!;
  const g = ctx.createLinearGradient(0, 0, w, h);
  g.addColorStop(0, def.base[0]);
  g.addColorStop(1, def.base[1]);
  ctx.fillStyle = g;
  ctx.fillRect(0, 0, w, h);
  for (const b of def.blobs) {
    const r = b.r * Math.max(w, h);
    const rg = ctx.createRadialGradient(b.x * w, b.y * h, 0, b.x * w, b.y * h, r);
    rg.addColorStop(0, b.c);
    rg.addColorStop(1, b.c + "00");
    ctx.fillStyle = rg;
    ctx.fillRect(0, 0, w, h);
  }
  return new Promise((res, rej) => cv.toBlob((b) => (b ? res(URL.createObjectURL(b)) : rej(new Error("paint failed"))), "image/jpeg", 0.92));
}

function stockDef(q: string, i: number): Def {
  const h = hash(`${q}:${i}`);
  const hue = (n: number) => `hsl(${(h * (n + 3)) % 360} ${55 + ((h >> n) % 30)}% ${35 + ((h >> (n + 2)) % 45)}%)`;
  const hex = (css: string) => {
    const c = document.createElement("canvas").getContext("2d")!;
    c.fillStyle = css;
    return c.fillStyle as string;
  };
  return { id: `stock-${h}`, name: q, base: [hex(hue(1)), hex(hue(2))], blobs: [0, 1, 2, 3].map((n) => ({ x: ((h >> n) % 100) / 100, y: ((h >> (n + 4)) % 100) / 100, r: 0.35 + ((h >> (n + 8)) % 30) / 100, c: hex(hue(n + 3)) })) };
}

import { all, tx } from "./idb";
import { RECIPES, renderRecipe } from "./recipe";

interface Stored { id: string; name: string; blob: Blob; source: "upload" | "unsplash"; credit?: { author: string; link: string } }

/** The in-browser stand-in for the server's wallpaper storage. */
export class FixtureLibrary implements WallpaperLibrary {
  private built: Promise<Wallpaper[]> | null = null;

  private async builtins(): Promise<Wallpaper[]> {
    this.built ??= (async () => {
      // Generated looks are only listed here: their pictures are rendered when one is chosen or shown in the picker (`resolve`, `thumbOf`), not at start.
      const looks = [{ id: "haze", name: "Haze" }, ...RECIPES].map((r) => ({ id: r.id, name: r.name, source: "builtin" as const, url: "", thumb: "", recipe: r.id }));
      const credits: { id: string; name: string; author: string; link: string }[] = await fetch(`${BASE}wallpapers/credits.json`).then((r) => (r.ok ? r.json() : [])).catch(() => []);
      const photos = credits.map((c) => ({ id: c.id, name: `Photo by ${c.author}`, source: "builtin" as const, url: `${BASE}wallpapers/${c.id}-light.jpg`, urlDark: `${BASE}wallpapers/${c.id}.jpg`, thumb: `${BASE}wallpapers/${c.id}-thumb.jpg`, credit: { author: c.author, link: c.link } }));
      // Photos first (the defaults), then the generated looks.
      return [...photos, ...looks];
    })();
    return this.built;
  }

  private rendered = new Map<string, Promise<string>>();
  private once(key: string, make: () => Promise<string>): Promise<string> {
    let p = this.rendered.get(key);
    if (!p) this.rendered.set(key, (p = make()));
    return p;
  }
  async resolve(w: Wallpaper, scheme: "light" | "dark"): Promise<string> {
    if (!w.recipe) return scheme === "dark" && w.urlDark ? w.urlDark : w.url;
    return this.once(`${w.recipe}:${scheme}`, () => renderRecipe(w.recipe!, scheme, 1600, 900));
  }
  async thumbOf(w: Wallpaper): Promise<string> {
    return w.recipe ? this.once(`${w.recipe}:thumb`, () => renderRecipe(w.recipe!, "light", 240, 150)) : w.thumb;
  }

  async list(): Promise<Wallpaper[]> {
    let mine: Stored[] = [];
    try {
      mine = await all<Stored>("w");
    } catch {
      /* no IndexedDB (private mode): built-ins only */
    }
    const own = mine.map<Wallpaper>((m) => { const u = URL.createObjectURL(m.blob); return { id: m.id, name: m.name, source: m.source, url: u, thumb: u, credit: m.credit, bytes: m.blob.size }; });
    return [...(await this.builtins()), ...own];
  }

  async upload(file: File): Promise<Wallpaper> {
    if (!file.type.startsWith("image/")) throw new Error("That file is not a picture.");
    const id = `up-${Date.now().toString(36)}`;
    await tx("w", "readwrite", (s) => s.put({ id, name: file.name.replace(/\.[^.]+$/, ""), blob: file, source: "upload" } satisfies Stored, id));
    const u = URL.createObjectURL(file);
    return { id, name: file.name.replace(/\.[^.]+$/, ""), source: "upload", url: u, thumb: u };
  }

  async fromLink(link: string): Promise<Wallpaper> {
    let u: URL;
    try { u = new URL(link.trim()); } catch { throw new Error("That is not a link."); }
    if (u.protocol !== "https:" && u.protocol !== "http:") throw new Error("Only http and https links work.");
    let blob: Blob;
    try {
      const r = await fetch(u.href);
      if (!r.ok) throw new Error(String(r.status));
      blob = await r.blob();
    } catch {
      throw new Error("The browser could not fetch that picture (its site may not allow it). The server will download links itself.");
    }
    if (!blob.type.startsWith("image/")) throw new Error("That link is not a picture.");
    if (blob.size > 15 * 1024 * 1024) throw new Error("That picture is larger than 15 MB.");
    const name = decodeURIComponent(u.pathname.split("/").filter(Boolean).pop() ?? "Picture").replace(/\.[^.]+$/, "").slice(0, 40) || "Picture";
    const id = `up-${Date.now().toString(36)}`;
    await tx("w", "readwrite", (s) => s.put({ id, name, blob, source: "upload" } satisfies Stored, id));
    const o = URL.createObjectURL(blob);
    return { id, name, source: "upload", url: o, thumb: o, bytes: blob.size };
  }

  async remove(id: string): Promise<void> {
    await tx("w", "readwrite", (s) => s.delete(id));
  }

  async search(query: string): Promise<StockResult[]> {
    const q = query.trim();
    if (!q) return [];
    const authors = ["Mira Okafor", "Jonas Lindqvist", "Aiko Tanaka", "Lucas Ferreira", "Noor Rahman", "Elena Petrova"];
    return Promise.all(
      authors.map(async (author, i) => ({
        id: `${q}:${i}`,
        description: `${q}, photo ${i + 1} (a generated stand-in for an Unsplash photo)`,
        thumb: await paint(stockDef(q, i), 240, 150),
        credit: { author, link: "https://unsplash.com/" },
      })),
    );
  }

  async save(r: StockResult): Promise<Wallpaper> {
    const [q, i] = [r.id.slice(0, r.id.lastIndexOf(":")), Number(r.id.slice(r.id.lastIndexOf(":") + 1))];
    const url = await paint(stockDef(q, i), 1600, 1000);
    const blob = await (await fetch(url)).blob();
    const id = `us-${hash(r.id).toString(36)}`;
    await tx("w", "readwrite", (s) => s.put({ id, name: q, blob, source: "unsplash", credit: r.credit } satisfies Stored, id));
    return { id, name: q, source: "unsplash", url, thumb: r.thumb, credit: r.credit };
  }
}
