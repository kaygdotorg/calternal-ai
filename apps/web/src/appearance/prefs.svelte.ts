/**
 * The person's appearance settings: wallpaper, glass strengths, accent, fonts. Persisted in localStorage here; the server will store them per
 * account (and the wallpapers and fonts they refer to). `appearance.start()` applies them and keeps the CSS variables current.
 */
import { applyScheme, currentScheme, onScheme, schemePreference, type SchemePreference } from "@calternal/ui";
import { applyFonts, DEFAULT_FONTS, FixtureFonts, type FontLibrary, type FontPrefs } from "./fonts";
import { FixtureLibrary, type Wallpaper, type WallpaperLibrary } from "./library";
import { onWallpaperColours, setWallpaper, updateWash, type WallpaperColours } from "./wallpaper";

/** How the accent colour is chosen. */
export type AccentMode = "default" | "wallpaper" | "custom";

/** Everything the settings sheet edits. */
export interface Prefs {
  /** A library id, `"auto"` (Dune in light, Aurora in dark) or null for none. */
  wallpaperId: string | null;
  /** The sidebars' copy of the wallpaper: blur radius in px, and the strength of the tint over it (percent). */
  side: { blur: number; tint: number };
  /** The middle card's copy: blur radius, tint strength and dim strength (percent). */
  card: { blur: number; tint: number; dim: number };
  accent: AccentMode;
  accentColor: string;
  fonts: FontPrefs;
}

/** The defaults, after the plugin's: a lightly blurred wallpaper behind the sidebars, a heavily blurred, deeper card. */
export const DEFAULTS: Prefs = {
  wallpaperId: "auto",
  side: { blur: 12, tint: 35 },
  card: { blur: 75, tint: 85, dim: 30 },
  accent: "default",
  accentColor: "#c26a12",
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
    const id = this.prefs.wallpaperId === "auto" ? (currentScheme() === "dark" ? "aurora" : "dune") : this.prefs.wallpaperId;
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
    if (accent === "custom") st.setProperty("--accent-base", accentColor);
    else if (accent === "wallpaper" && this.colours?.hue) st.setProperty("--accent-base", `oklch(0.62 ${Math.min(0.17, Math.max(0.09, this.colours.hue.c * 1.4)).toFixed(3)} ${this.colours.hue.h.toFixed(1)})`);
    else st.removeProperty("--accent-base");
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
