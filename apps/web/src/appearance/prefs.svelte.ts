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
  /** The sidebars' copy of the wallpaper: blur radius in px, and the strength of the tint over it (percent). */
  side: { blur: number; tint: number };
  /** The middle card's copy: blur radius, tint strength and dim strength (percent). */
  card: { blur: number; tint: number; dim: number };
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
  side: { blur: 12, tint: 35 },
  card: { blur: 75, tint: 85, dim: 30 },
  themeId: null,
  presetId: tokens.appearance["default-preset"],
  accent: "wallpaper",
  accentColor: "#3b6ea8",
  brand: "Calternal",
  streamText: false,
  fonts: DEFAULT_FONTS,
};

const KEY = "cal.appearance";

function read(): Prefs {
  try {
    const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as Partial<Prefs> | null;
    return s ? { ...DEFAULTS, ...s, side: { ...DEFAULTS.side, ...s.side }, card: { ...DEFAULTS.card, ...s.card }, fonts: { ...DEFAULTS.fonts, ...s.fonts } } : structuredClone(DEFAULTS);
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
    return this.wallpapers.find((w) => w.id === id);
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
    st.setProperty("--side-tint-a", String(p.side.tint / 100));
    st.setProperty("--card-tint-a", String(p.card.tint / 100));
    st.setProperty("--card-dim-a", String(p.card.dim / 100));
    const w = this.wallpaper;
    setWallpaper(w ? (currentScheme() === "dark" && w.urlDark ? w.urlDark : w.url) : null, { sideBlur: p.side.blur, cardBlur: p.card.blur });
    applyTheme(p.themeId ? (presetById(p.themeId)?.theme ?? null) : null, currentScheme());
    updateWash(p.side.tint / 100);
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
      x.card = { ...x.card, tint: pre.material === "paper" ? 100 : 85, dim: pre.material === "paper" ? 0 : 30 };
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
