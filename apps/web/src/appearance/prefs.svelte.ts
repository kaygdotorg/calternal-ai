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
import { onWallpaperColours, setWallpaper, updateWash, type WallpaperColours } from "./wallpaper";

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
  themeId: null,
  presetId: tokens.appearance["default-preset"],
  accent: "wallpaper",
  accentColor: "#3b6ea8",
  brand: "Calternal",
  streamText: false,
  fonts: DEFAULT_FONTS,
};

/** What each switch turns on. The sidebar tint and the card tint are floors: the wash is raised above them where the wallpaper needs it for contrast. */
const SIDE_BLUR = 12, CARD_BLUR = 75, SIDE_TINT = 0.35, CARD_TINT = 0.85, CARD_DIM = 0.3;
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
  private colours: WallpaperColours | null = null;
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
    const base = { side: m.tint ? SIDE_TINT : 0, card: m.tint ? CARD_TINT : 0 };
    st.setProperty("--side-tint-a", String(base.side));
    st.setProperty("--card-tint-a", String(base.card));
    st.setProperty("--card-dim-a", String(m.dim ? CARD_DIM : 0));
    st.setProperty("--ui-scale", String(Math.min(150, Math.max(80, p.uiSize)) / 100));
    const w = this.wallpaper;
    setWallpaper(w ? (currentScheme() === "dark" && w.urlDark ? w.urlDark : w.url) : null, { sideBlur: m.blur ? SIDE_BLUR : 0, cardBlur: m.blur ? CARD_BLUR : 0 });
    applyTheme(p.themeId ? (presetById(p.themeId)?.theme ?? null) : null, currentScheme());
    updateWash(base.side, base.card);
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
