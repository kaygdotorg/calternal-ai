/**
 * Font choice as a setting, after the owner's calternal-ui plugin (`shared/fonts.ts`, `server/fonts-*.ts`): a family per role (interface, chat, code, terminal and the wordmark)
 * from Google Fonts, Fontsource, Fontshare, the person's own files, or the system font. The server will download a chosen family once, store its
 * files and serve them from its own storage, so pages never load from those sites. `FixtureFonts` stands in: its catalogues are short hand-written
 * lists, and only families that ship with the app (or that the person uploads) can actually be drawn without the server.
 * Defaults: Google Sans Flex for the interface and chat, Cascadia Code for code and the terminal (Maple Mono is selectable and never preloaded).
 */
import { all, tx } from "./idb";

/** Where a family comes from. */
export type FontSource = "google" | "fontsource" | "fontshare" | "upload" | "system";

/** The places a font applies: interface, chat, code, terminal, and the wordmark. */
export type FontRole = "ui" | "chat" | "code" | "terminal" | "brand";

/** Every role, in the order Settings lists them, with the CSS variable each one sets and whether it takes monospaced families only. The one table the loader and the picker both read. */
export const ROLES: { role: FontRole; label: string; variable: string; mono: boolean }[] = [
  { role: "ui", label: "Interface", variable: "--font-ui", mono: false },
  { role: "chat", label: "Chat", variable: "--font-chat", mono: false },
  { role: "code", label: "Code", variable: "--font-mono", mono: true },
  { role: "terminal", label: "Terminal", variable: "--font-terminal", mono: true },
  { role: "brand", label: "Wordmark", variable: "--font-brand", mono: false },
];

/** A choice for one role. */
export interface FontChoice {
  /** The font family name as CSS knows it. */
  family: string;
  /** Where the family comes from, which decides how it is loaded: a catalogue (google, fontsource, fontshare), the person's upload, or the system. */
  source: FontSource;
}

/** The person's choices, one per role. */
export type FontPrefs = Record<FontRole, FontChoice>;

/** The defaults: the existing app's own faces. */
export const DEFAULT_FONTS: FontPrefs = {
  ui: { family: "Google Sans Flex", source: "google" },
  chat: { family: "Google Sans Flex", source: "google" },
  code: { family: "Cascadia Code", source: "fontsource" },
  terminal: { family: "Cascadia Code", source: "fontsource" },
  /** The wordmark at the top of the sidebar: a cursive face, as in the owner's calternal-ui plugin. */
  brand: { family: "Dancing Script", source: "google" },
};

/** One family in a catalogue. `ready` is false when only the server could fetch it (fixture: not downloaded). */
export interface FontInfo {
  /** The family name as CSS knows it. */
  family: string;
  /** Where the family comes from: a catalogue, an upload or the system. */
  source: FontSource;
  /** True for a monospace family: offered for code, never for text. */
  mono: boolean;
  /** False when the family is only listed (not downloaded yet): it cannot be drawn until it has been. */
  ready: boolean;
}

/** The font catalogues and storage the settings sheet talks to. */
export interface FontLibrary {
  /** Families of one source matching `query` (empty query: the popular ones), monospaced ones only when `mono`. */
  search(source: FontSource, query: string, mono: boolean): Promise<FontInfo[]>;
  /** Makes a family drawable (the server downloads and stores it; here: only shipped or own families succeed). Rejects with a plain reason. */
  install(font: FontInfo): Promise<void>;
  /** Stores font files the person is licensed to use under one family name. Weights and italics are read from file names ("Bold", "SemiboldItalic"). */
  addOwn(name: string, files: File[]): Promise<FontInfo>;
  /** The person's own families, to show under Upload. */
  own(): Promise<FontInfo[]>;
}

/** The CSS stack for the system font: the text faces for interface and chat, the monospaced ones for code. */
export const systemStack = (mono: boolean): string =>
  mono ? `ui-monospace, "SF Mono", SFMono-Regular, Menlo, Consolas, monospace` : `-apple-system, BlinkMacSystemFont, system-ui, "Segoe UI", Roboto, sans-serif`;

/** How each source is named in the picker. */
export const SOURCE_LABEL: Record<FontSource, string> = { google: "Google", fontsource: "Fontsource", fontshare: "Fontshare", upload: "Upload", system: "System" };

const BUNDLED = new Set(["Google Sans Flex", "Cascadia Code", "Maple Mono", "Dancing Script"]);
const CATALOGUE: Omit<FontInfo, "ready">[] = [
  ...["Google Sans Flex", "Inter", "Roboto", "Source Serif 4", "Lora", "Dancing Script", "Pacifico", "Caveat", "Nunito", "DM Sans"].map((family) => ({ family, source: "google" as const, mono: false })),
  ...["JetBrains Mono", "Fira Code", "Source Code Pro", "IBM Plex Mono"].map((family) => ({ family, source: "google" as const, mono: true })),
  ...["Geist Sans", "iA Writer Quattro", "Open Runde"].map((family) => ({ family, source: "fontsource" as const, mono: false })),
  ...["Cascadia Code", "Maple Mono", "Monaspace Neon", "Iosevka", "Geist Mono"].map((family) => ({ family, source: "fontsource" as const, mono: true })),
  ...["Satoshi", "General Sans", "Switzer", "Clash Display"].map((family) => ({ family, source: "fontshare" as const, mono: false })),
];

/** Weight and italic from a file name like `Foo-SemiBoldItalic.ttf`. */
export function faceOf(file: string): { weight: number; italic: boolean } {
  const n = file.toLowerCase().replace(/[^a-z]/g, "");
  const table: [string, number][] = [["thin", 100], ["extralight", 200], ["ultralight", 200], ["light", 300], ["medium", 500], ["semibold", 600], ["demibold", 600], ["extrabold", 800], ["bold", 700], ["black", 900], ["heavy", 900]];
  return { weight: table.find(([k]) => n.includes(k))?.[1] ?? 400, italic: n.includes("italic") || n.includes("oblique") };
}

interface StoredFont { id: string; family: string; file: string; blob: Blob }

/** The in-browser stand-in for the server's font catalogues and storage. */
export class FixtureFonts implements FontLibrary {
  async search(source: FontSource, query: string, mono: boolean): Promise<FontInfo[]> {
    if (source === "system") return [{ family: "System", source, mono, ready: true }];
    if (source === "upload") return this.own();
    const q = query.trim().toLowerCase();
    return CATALOGUE.filter((f) => f.source === source && f.mono === mono && f.family.toLowerCase().includes(q)).map((f) => ({ ...f, ready: BUNDLED.has(f.family) }));
  }

  async install(font: FontInfo): Promise<void> {
    if (font.source === "system" || font.source === "upload" || BUNDLED.has(font.family)) return;
    throw new Error(`${font.family} comes from ${SOURCE_LABEL[font.source]}. The server downloads and stores it; this preview has no server, so it cannot be drawn here.`);
  }

  async addOwn(name: string, files: File[]): Promise<FontInfo> {
    const family = name.trim();
    if (!family || !files.length) throw new Error("Give the font a name and choose at least one file.");
    for (const f of files) {
      if (!/\.(woff2?|ttf|otf)$/i.test(f.name)) throw new Error(`${f.name} is not a font file (woff2, woff, ttf or otf).`);
      const id = `${family}/${f.name}`;
      await tx("f", "readwrite", (s) => s.put({ id, family, file: f.name, blob: f } satisfies StoredFont, id));
    }
    await loadOwn(family);
    return { family, source: "upload", mono: false, ready: true };
  }

  async own(): Promise<FontInfo[]> {
    try {
      const fam = new Set((await all<StoredFont>("f")).map((f) => f.family));
      return [...fam].map((family) => ({ family, source: "upload" as const, mono: false, ready: true }));
    } catch {
      return [];
    }
  }
}

const registered = new Set<string>();

/** Registers a stored own family with the browser (once per page). */
export async function loadOwn(family: string): Promise<void> {
  if (registered.has(family)) return;
  for (const f of (await all<StoredFont>("f")).filter((x) => x.family === family)) {
    const { weight, italic } = faceOf(f.file);
    const face = new FontFace(family, await f.blob.arrayBuffer(), { weight: String(weight), style: italic ? "italic" : "normal", display: "block" });
    document.fonts.add(await face.load());
  }
  registered.add(family);
}

/**
 * Applies the choices as CSS variables on the root, one per role (the `ROLES` table: `--font-ui`, `--font-chat`, `--font-mono`, `--font-terminal`, `--font-brand`). The defaults remove the overrides, so the
 * scheme's own family stack (light, or Dark with its GRAD axis) applies. A chosen family is used for both schemes. Own fonts are registered first.
 */
export async function applyFonts(p: FontPrefs): Promise<void> {
  const st = document.documentElement.style;
  const set = async (role: FontRole, name: string) => {
    const c = p[role];
    const def = DEFAULT_FONTS[role];
    if (c.family === def.family && c.source === def.source) return void st.removeProperty(name);
    if (c.source === "upload") await loadOwn(c.family);
    st.setProperty(name, c.source === "system" ? systemStack(ROLES.find((r) => r.role === role)!.mono) : `"${c.family}"`);
  };
  await Promise.all(ROLES.map((r) => set(r.role, r.variable)));
}
