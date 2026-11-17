/**
 * The person's appearance settings: wallpaper, glass strengths, accent, fonts. Persisted in localStorage here; the server will store them per
 * account (and the wallpapers and fonts they refer to). `appearance.start()` applies them and keeps the CSS variables current.
 */
import { applyScheme, currentScheme, onScheme, schemePreference, type SchemePreference } from "@calternal/ui";
import { applyFonts, DEFAULT_FONTS, FixtureFonts, type FontLibrary, type FontPrefs } from "./fonts";
import { FixtureLibrary, type Wallpaper, type WallpaperLibrary } from "./library";
import { accentFor } from "./accent";
import { onWallpaperColours, setWallpaper, updateWash, type WallpaperColours } from "./wallpaper";

/** How the accent colour is chosen. */
export type AccentMode = "wallpaper" | "custom";

/** Everything the settings sheet edits. */
export interface Prefs {
  /** A library id, `"auto"` (Golden field in light, Storm in dark) or null for none. */
  wallpaperId: string | null;
  /** The sidebars' copy of the wallpaper: blur radius in px, and the strength of the tint over it (percent). */
  side: { blur: number; tint: number };
  /** The middle card's copy: blur radius, tint strength and dim strength (percent). */
  card: { blur: number; tint: number; dim: number };
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
    const id = this.prefs.wallpaperId === "auto" ? (currentScheme() === "dark" ? "storm-sea" : "golden-field") : this.prefs.wallpaperId;
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
    onScheme(() => (this.prefs.wallpaperId === "auto" ? this.apply() : updateWash(this.prefs.side.tint / 100)));
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
    setWallpaper(this.wallpaper?.url ?? null, { sideBlur: p.side.blur, cardBlur: p.card.blur });
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
