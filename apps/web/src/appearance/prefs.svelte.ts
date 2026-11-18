/**
 * The person's appearance settings: wallpaper, glass strengths, accent, fonts. Persisted in localStorage here; the server will store them per
 * account (and the wallpapers and fonts they refer to). `appearance.start()` applies them and keeps the CSS variables current.
 */
import { applyScheme, currentScheme, onScheme, schemePreference, type SchemePreference } from "@calternal/ui";
import { applyFonts, DEFAULT_FONTS, FixtureFonts, type FontLibrary, type FontPrefs } from "./fonts";
import { FixtureLibrary, type Wallpaper, type WallpaperLibrary } from "./library";
import tokens from "../../../../packages/ui/tokens/tokens.json" with { type: "json" };
import { accentFor } from "./accent";
import { PRESETS, presetById } from "./presets";
import { applyTheme } from "./theme";
import { setFloor } from "./veil";
import { setClarity } from "./wallpaper";
import { onWallpaperColours, setCraftLight, setFlags, setWallpaper, updateVeil, type WallpaperColours } from "./wallpaper";

/** How the accent colour is chosen. */
export type AccentMode = "wallpaper" | "custom" | "theme";

/** Everything the settings sheet edits. */
export interface Prefs {
  /** A library id, `"auto"` (the default look, `appearance.default-wallpaper` in tokens.json) or null for none. */
  wallpaperId: string | null;
  /**
   * The glass, as three switches (no sliders). Each one's strength is computed, not chosen: the sidebar wash and the card's tint are raised from the
   * wallpaper's own brightness under each surface until text passes contrast in the current scheme (`washFor`), whatever the switches say.
   */
  materials: { blur: boolean; dim: boolean; tint: boolean };
  /** The UI size, percent: one scale for type, spacing and controls together (`--ui-scale`). */
  uiSize: number;
  /** The order the person chose for the apps in the switcher (app kinds, first to last); apps not listed follow in their own order. */
  appOrder: string[];
  /** The theme (palette) in use: a preset id whose palette applies, or null for the built-in tokens. Independent of the wallpaper. */
  themeId: string | null;
  /** The preset last applied, for the gallery's selected tile. */
  presetId: string | null;
  /** Where the accent colour comes from: the wallpaper, a preset's own, or a custom colour. */
  accent: AccentMode;
  /** The custom accent as `#rrggbb`; used only when `accent` is `custom`. */
  accentColor: string;
  /** The wordmark at the top of the sidebar: its text (the font is `fonts.brand`). */
  brand: string;
  /** Whether an agent's message streams in as it is written. Off by default: a working state and the steps show, then the finished message appears at once. */
  streamText: boolean;
  /** The font choices per role (text, headings, code); a family that is not loaded falls back to the shipped one. */
  fonts: FontPrefs;
  /** Whether the interface animates. The system's reduce-motion setting is always honoured (the motion tokens shorten); this is the person's own off switch on top of it. */
  motion: boolean;
  /** The chat column on a desktop: `wide` (the default, from the calternal-ui plugin's wide chat) fills the card, `centered` keeps a readable column. */
  chatWidth: "wide" | "centered";
  /**
   * TEMPORARY, for the owner's side-by-side comparison (Settings, Appearance, Compare); removed once decided. `floor`: the readability floor the veil is solved to (`current` 7:1 with text that
   * flips, or `low` 4.5:1 that shows more wallpaper). `light`: the light scheme's surfaces (`derived`: the lighter photo with the solved veil, or `craft`: the unchanged photo under 0.92 white).
   */
  compare: { floor: "current" | "low"; light: "derived" | "craft"; /** Clear to tinted, 0 to 1: how much of the wallpaper shows through the veils. Null: from the system (reduced transparency or more contrast ask for tinted). */ clarity: number | null };
}

/** The defaults, after the plugin's: a lightly blurred wallpaper behind the sidebars, a heavily blurred, deeper card. */
export const DEFAULTS: Prefs = {
  wallpaperId: "auto",
  materials: { blur: true, dim: true, tint: true },
  uiSize: 100,
  appOrder: [],
  themeId: null,
  presetId: tokens.appearance["default-preset"],
  accent: "wallpaper",
  accentColor: "#3b6ea8",
  brand: "Calternal",
  streamText: false,
  fonts: DEFAULT_FONTS,
  motion: true,
  chatWidth: "wide",
  compare: { floor: "current", light: "derived", clarity: null },
};

/** What Blur turns on: the blur radii (px) of the sidebars' and the card's copies of the picture. Tint and Dim have no fixed strength: the veil is solved (veil.ts). */
const SIDE_BLUR = 12, CARD_BLUR = 75;
const KEY = "cal.appearance";

function read(): Prefs {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<Prefs> | null;
    return s ? { ...DEFAULTS, ...s, materials: { ...DEFAULTS.materials, ...s.materials }, fonts: { ...DEFAULTS.fonts, ...s.fonts }, compare: { ...DEFAULTS.compare, ...s.compare } } : structuredClone(DEFAULTS);
  } catch {
    return structuredClone(DEFAULTS);
  }
}

class Appearance {
  prefs = $state<Prefs>(read());
  wallpapers = $state<Wallpaper[]>([]);
  scheme = $state<SchemePreference>(schemePreference());
  library: WallpaperLibrary = new FixtureLibrary();
  fonts: FontLibrary = new FixtureFonts();
  /** Thumbnails of generated looks, filled in when the picker or the preset gallery first needs them. */
  thumbs = $state<Record<string, string>>({});
  private colours: WallpaperColours | null = null;
  private applySeq = 0;
  private started = false;

  /** The chosen wallpaper, if the library has it. */
  get wallpaper(): Wallpaper | undefined {
    const id = this.prefs.wallpaperId === "auto" ? tokens.appearance["default-wallpaper"] : this.prefs.wallpaperId;
    // The default is a photo fetched at build time; if it is not there (an offline build), the generated look stands in.
    return this.wallpapers.find((w) => w.id === id) ?? (this.prefs.wallpaperId === "auto" ? this.wallpapers.find((w) => w.id === "haze") : undefined);
  }

  /** Loads the library and applies everything. Call once at start. */
  async start(): Promise<void> {
    if (this.started) return;
    this.started = true;
    onWallpaperColours((c) => {
      this.colours = c;
      this.applyAccent();
    });
    // A scheme change re-renders what depends on it: a generated look's dark rendition, a theme's palette, the wash.
    onScheme(() => this.apply());
    this.wallpapers = await this.library.list();
    this.apply();
  }

  /** Clear to tinted, 0 to 1. The person's choice, else from the system: reduced transparency or more contrast mean tinted (1); otherwise the middle (0.5), where the solved veil is untouched. */
  get clarity(): number {
    const c = this.prefs.compare.clarity;
    if (typeof c === "number") return Math.round(Math.min(1, Math.max(0, c)) * 4) / 4; // five steps: Clear, Light, Balanced, Dense, Tinted
    return matchMedia("(prefers-reduced-transparency: reduce)").matches || matchMedia("(prefers-contrast: more)").matches ? 1 : 0.5;
  }

  /** Writes the CSS variables and starts whatever must be redone for the current preferences. Stores them only when `save` (a change the person made): a first visit keeps no stored setting, so the defaults stay the defaults. */
  apply(save = false): void {
    const p = this.prefs;
    const st = document.documentElement.style;
    const m = p.materials;
    st.setProperty("--ui-scale", String(Math.min(150, Math.max(80, p.uiSize)) / 100));
    setFlags(m);
    // The tiny wallpaper index.html paints first belongs to the default only: any other choice (or none) takes it away.
    if (p.wallpaperId !== "auto") delete document.documentElement.dataset.cheap;
    const w = this.wallpaper;
    const look = { sideBlur: m.blur ? SIDE_BLUR : 0, cardBlur: m.blur ? CARD_BLUR : 0 };
    const mine = ++this.applySeq;
    const scheme = currentScheme();
    if (p.chatWidth === "wide") document.documentElement.dataset.chatWide = ""; else delete document.documentElement.dataset.chatWide;
    if (p.motion) delete document.documentElement.dataset.motion; else document.documentElement.dataset.motion = "off";
    // The comparison: a lower readability floor, and Craft's light surfaces on the unchanged photograph (a photo's unchanged rendition is its dark-scheme file).
    setFloor(p.compare.floor);
    setClarity(this.clarity);
    const craft = p.compare.light === "craft" && scheme === "light";
    setCraftLight(craft);
    const picture = w && !w.recipe && craft ? "dark" : scheme;
    if (!w) setWallpaper(null, look);
    else void (this.library.resolve?.(w, picture) ?? Promise.resolve(picture === "dark" && w.urlDark ? w.urlDark : w.url)).then((src) => { if (mine === this.applySeq) setWallpaper(src, look); });
    applyTheme(p.themeId ? (presetById(p.themeId)?.theme ?? null) : null, currentScheme());
    updateVeil(m);
    this.applyAccent();
    void applyFonts(p.fonts);
    if (!save) return;
    try {
      localStorage.setItem(KEY, JSON.stringify(p));
    } catch {
      /* private mode: the choice lasts for this page only */
    }
  }

  private applyAccent(): void {
    const st = document.documentElement.style;
    const { accent, accentColor } = this.prefs;
    // From the wallpaper by default (guarded for contrast in both schemes, accent.ts); a custom colour is used as chosen.
    if (accent === "theme") return; // the theme sets --accent-base itself (appearance/theme.ts)
    if (accent === "custom") st.setProperty("--accent-base", accentColor);
    else st.setProperty("--accent-base", accentFor(this.colours?.hue?.h ?? null, this.colours?.hue?.c ?? null).hex);
  }

  /** Changes preferences with `f` and applies them. */
  change(f: (p: Prefs) => void): void {
    f(this.prefs);
    this.apply(true);
  }

  /** Colour scheme preference (System, Light, Dark). */
  setScheme(s: SchemePreference): void {
    this.scheme = s;
    applyScheme(s);
  }

  /** Applies a preset: its backdrop and its palette together, with the material it asks for. Both stay changeable on their own afterwards. */
  applyPreset(id: string): void {
    const pre = presetById(id);
    if (!pre) return;
    this.change((x) => {
      x.presetId = id;
      x.themeId = pre.theme ? pre.id : null;
      x.wallpaperId = pre.wallpaper === "none" ? null : pre.wallpaper === tokens.appearance["default-wallpaper"] ? "auto" : pre.wallpaper;
      x.accent = pre.theme ? "theme" : pre.accentFromWallpaper ? "wallpaper" : "custom";
      x.materials = { ...x.materials, dim: pre.material !== "paper" };
    });
  }

  /** Restores every setting to its default. */
  reset(): void {
    this.prefs = structuredClone(DEFAULTS);
    this.apply(true);
  }

  /** Renders the thumbnails of generated looks that have none yet (when a picker opens). Each is rendered once. */
  async ensureThumbs(): Promise<void> {
    for (const w of this.wallpapers) {
      if (w.thumb || this.thumbs[w.id] || !this.library.thumbOf) continue;
      // One picture per turn of the event loop, after the next frame: painting a generated look is real work, and opening Settings must not spend it all inside one frame.
      await new Promise<void>((r) => requestAnimationFrame(() => setTimeout(r, 0)));
      const t = await this.library.thumbOf(w);
      this.thumbs = { ...this.thumbs, [w.id]: t };
    }
  }
  /** The thumbnail to draw for a wallpaper. */
  thumbOf(w: Wallpaper): string { return w.thumb || this.thumbs[w.id] || ""; }

  /** Re-reads the library (after an upload or removal). */
  async refresh(): Promise<void> {
    this.wallpapers = await this.library.list();
  }

  /** The scheme currently drawn. */
  get drawn() {
    return currentScheme();
  }
}

/** The shared appearance state. */
export const appearance = new Appearance();
