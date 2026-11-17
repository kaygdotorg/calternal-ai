/**
 * Where wallpapers come from: a small built-in set, the person's own uploads, and Unsplash search, all behind one adapter. The server will store
 * what is chosen (pages never load from Unsplash or any other site while the app is in use: the server downloads a picked photo and serves it
 * from its own storage). `FixtureLibrary` is the in-browser stand-in: the built-ins are painted from palettes here, uploads live in IndexedDB,
 * and "Unsplash" results are procedurally generated pictures with fake credits. Fixture-only; nothing here ships with the real client.
 */

/** One picture the person can use. `url` is something an `<img>` can load (a blob URL here; a same-origin file from the real server). */
export interface Wallpaper {
  id: string;
  name: string;
  source: "builtin" | "upload" | "unsplash";
  url: string;
  /** A small version for the picker. */
  thumb: string;
  /** For an Unsplash photo: who took it and where it is, as their licence asks. */
  credit?: { author: string; link: string };
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
  /** Stores an image file the person chose. */
  upload(file: File): Promise<Wallpaper>;
  /** Removes an upload or saved photo (built-ins cannot be removed). */
  remove(id: string): Promise<void>;
  /** Searches Unsplash. Needs the server's access key; without it the real implementation returns an empty list and says why. */
  search(query: string): Promise<StockResult[]>;
  /** The server downloads the photo and stores it; returns it as a library wallpaper. */
  save(result: StockResult): Promise<Wallpaper>;
}

interface Blob2 { x: number; y: number; r: number; c: string }
interface Def { id: string; name: string; base: [string, string]; blobs: Blob2[] }

/** The built-in set: soft, saturated colour fields, each with a light and a dark region so both schemes have something to work with. */
const BUILTIN: Def[] = [
  { id: "dune", name: "Dune", base: ["#f3d9b8", "#e8a77a"], blobs: [{ x: 0.2, y: 0.25, r: 0.55, c: "#fff1d6" }, { x: 0.8, y: 0.7, r: 0.6, c: "#d97c5a" }, { x: 0.55, y: 1, r: 0.5, c: "#8e4a3b" }, { x: 0.95, y: 0.1, r: 0.35, c: "#f7c58f" }] },
  { id: "lagoon", name: "Lagoon", base: ["#bfe6e4", "#4aa3b0"], blobs: [{ x: 0.15, y: 0.2, r: 0.5, c: "#eafbf5" }, { x: 0.75, y: 0.35, r: 0.55, c: "#2f7fa3" }, { x: 0.4, y: 0.95, r: 0.6, c: "#1d4f6e" }, { x: 0.95, y: 0.9, r: 0.35, c: "#7ed0c0" }] },
  { id: "aurora", name: "Aurora", base: ["#1d2b4f", "#3a2a6b"], blobs: [{ x: 0.25, y: 0.3, r: 0.5, c: "#41d1b0" }, { x: 0.7, y: 0.2, r: 0.5, c: "#7b5bd6" }, { x: 0.55, y: 0.8, r: 0.55, c: "#2b6fc4" }, { x: 0.05, y: 0.9, r: 0.35, c: "#0f1630" }] },
  { id: "ember", name: "Ember", base: ["#f2c6a0", "#d96a4a"], blobs: [{ x: 0.2, y: 0.8, r: 0.55, c: "#f6a06a" }, { x: 0.75, y: 0.25, r: 0.5, c: "#c43d4a" }, { x: 0.5, y: 0.5, r: 0.4, c: "#ffd9a8" }, { x: 1, y: 1, r: 0.4, c: "#6b2a3a" }] },
  { id: "orchid", name: "Orchid", base: ["#efd6f0", "#b58ad8"], blobs: [{ x: 0.2, y: 0.2, r: 0.5, c: "#fbeaf8" }, { x: 0.8, y: 0.6, r: 0.55, c: "#8b5fc7" }, { x: 0.4, y: 0.9, r: 0.5, c: "#e58fc0" }, { x: 0.95, y: 0.05, r: 0.3, c: "#a9c7f2" }] },
  { id: "moss", name: "Moss", base: ["#d7e3c3", "#7f9f6a"], blobs: [{ x: 0.3, y: 0.2, r: 0.5, c: "#f1f4dc" }, { x: 0.75, y: 0.65, r: 0.55, c: "#4f7a4a" }, { x: 0.15, y: 0.9, r: 0.45, c: "#a9c47b" }, { x: 0.9, y: 0.1, r: 0.3, c: "#e9d9a4" }] },
  { id: "slate", name: "Slate", base: ["#3b4252", "#1f232e"], blobs: [{ x: 0.2, y: 0.25, r: 0.5, c: "#5d6b86" }, { x: 0.8, y: 0.75, r: 0.55, c: "#2a3350" }, { x: 0.6, y: 0.1, r: 0.35, c: "#7a8aa6" }, { x: 0.1, y: 1, r: 0.4, c: "#14161e" }] },
  { id: "paper", name: "Paper", base: ["#f6f1e8", "#e7ddcd"], blobs: [{ x: 0.2, y: 0.3, r: 0.5, c: "#fffaf0" }, { x: 0.85, y: 0.8, r: 0.5, c: "#d9c9ae" }, { x: 0.6, y: 0.1, r: 0.3, c: "#f0e3cf" }, { x: 0.1, y: 0.95, r: 0.35, c: "#e0d2bb" }] },
];

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

interface Stored { id: string; name: string; blob: Blob; source: "upload" | "unsplash"; credit?: { author: string; link: string } }

/** The in-browser stand-in for the server's wallpaper storage. */
export class FixtureLibrary implements WallpaperLibrary {
  private built: Promise<Wallpaper[]> | null = null;

  private async builtins(): Promise<Wallpaper[]> {
    this.built ??= Promise.all(
      BUILTIN.map(async (d) => ({ id: d.id, name: d.name, source: "builtin" as const, url: await paint(d, 1600, 1000), thumb: await paint(d, 240, 150) })),
    );
    return this.built;
  }

  async list(): Promise<Wallpaper[]> {
    let mine: Stored[] = [];
    try {
      mine = await all<Stored>("w");
    } catch {
      /* no IndexedDB (private mode): built-ins only */
    }
    const own = mine.map<Wallpaper>((m) => { const u = URL.createObjectURL(m.blob); return { id: m.id, name: m.name, source: m.source, url: u, thumb: u, credit: m.credit }; });
    return [...(await this.builtins()), ...own];
  }

  async upload(file: File): Promise<Wallpaper> {
    if (!file.type.startsWith("image/")) throw new Error("That file is not a picture.");
    const id = `up-${Date.now().toString(36)}`;
    await tx("w", "readwrite", (s) => s.put({ id, name: file.name.replace(/\.[^.]+$/, ""), blob: file, source: "upload" } satisfies Stored, id));
    const u = URL.createObjectURL(file);
    return { id, name: file.name.replace(/\.[^.]+$/, ""), source: "upload", url: u, thumb: u };
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
