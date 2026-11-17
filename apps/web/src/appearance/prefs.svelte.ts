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
import { onWallpaperColours, setFlags, setWallpaper, updateVeil, type WallpaperColours } from "./wallpaper";

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
  /** Where the app switcher lives: in the sidebar's footer, or as a capsule at the bottom of the middle card (an experiment). */
  switcher: "sidebar" | "card";
  /** With the switcher under the card: hide it until the pointer or focus reaches the card's bottom edge. Off by default (always visible). */
  switcherAutoHide: boolean;
  /** The theme (palette) in use: a preset id whose palette applies, or null for the built-in tokens. Independent of the wallpaper. */
  themeId: string | null;
  /** The preset last applied, for the gallery's selected tile. */
  presetId: string | null;
  accent: AccentMode;
  accentColor: string;
  /** The wordmark at the top of the sidebar: its text (the font is `fonts.brand`). */
  brand: string;
  /** Whether an agent's message streams in as it is written. Off by default: a working state and the steps show, then the finished message appears at once. */
  streamText: boolean;
  fonts: FontPrefs;
}

/** The defaults, after the plugin's: a lightly blurred wallpaper behind the sidebars, a heavily blurred, deeper card. */
export const DEFAULTS: Prefs = {
  wallpaperId: "auto",
  materials: { blur: true, dim: true, tint: true },
  uiSize: 100,
  switcher: "sidebar",
  switcherAutoHide: false,
  themeId: null,
  presetId: tokens.appearance["default-preset"],
  accent: "wallpaper",
  accentColor: "#3b6ea8",
  brand: "Calternal",
  streamText: false,
  fonts: DEFAULT_FONTS,
};

/** What Blur turns on: the blur radii (px) of the sidebars' and the card's copies of the picture. Tint and Dim have no fixed strength: the veil is solved (veil.ts). */
const SIDE_BLUR = 12, CARD_BLUR = 75;
const KEY = "cal.appearance";

function read(): Prefs {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<Prefs> | null;
    return s ? { ...DEFAULTS, ...s, materials: { ...DEFAULTS.materials, ...s.materials }, fonts: { ...DEFAULTS.fonts, ...s.fonts } } : structuredClone(DEFAULTS);
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

  /** Writes the CSS variables and starts whatever must be redone for the current preferences. */
  apply(): void {
    const p = this.prefs;
    const st = document.documentElement.style;
    const m = p.materials;
    st.setProperty("--ui-scale", String(Math.min(150, Math.max(80, p.uiSize)) / 100));
    setFlags(m);
    const w = this.wallpaper;
    const look = { sideBlur: m.blur ? SIDE_BLUR : 0, cardBlur: m.blur ? CARD_BLUR : 0 };
    const mine = ++this.applySeq;
    const scheme = currentScheme();
    if (!w) setWallpaper(null, look);
    else void (this.library.resolve?.(w, scheme) ?? Promise.resolve(scheme === "dark" && w.urlDark ? w.urlDark : w.url)).then((src) => { if (mine === this.applySeq) setWallpaper(src, look); });
    applyTheme(p.themeId ? (presetById(p.themeId)?.theme ?? null) : null, currentScheme());
    updateVeil(m);
    this.applyAccent();
    void applyFonts(p.fonts);
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
    this.apply();
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
    this.apply();
  }

  /** Renders the thumbnails of generated looks that have none yet (when a picker opens). Each is rendered once. */
  async ensureThumbs(): Promise<void> {
    for (const w of this.wallpapers) {
      if (w.thumb || this.thumbs[w.id] || !this.library.thumbOf) continue;
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
