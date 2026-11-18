/**
 * The canvas's two preferences, the ones the Settings app (Appearance) binds: how a window looks (`look`) and how wide a new window opens (`width`). Neither is a per-window choice any more (the window menu has no Look and no Width entries):
 * a window's width is set by dragging its handle, and the look is the person's one preference for every window. Kept for this browser (`cal.canvas.prefs`: experiment state, not userdata). Reading a field in a component or a `$derived` makes
 * it follow the preference; `setLook` and `setWidth` change and keep it.
 */
import type { Look } from "./layout";

/** The looks a person can choose and their names for a setting: the middle card as it is today, a transparent card (the paper at the density that keeps text readable), a solid one (the paper). */
export const LOOKS: { value: Look; label: string }[] = [{ value: "card", label: "Card" }, { value: "clear", label: "Transparent" }, { value: "solid", label: "Solid" }];
/** The widths a new window can open at, as a share of the canvas; `null` is automatic (one window fills it, two share it, more keep half). */
export const OPEN_WIDTHS: { value: number | null; label: string }[] = [{ value: null, label: "Automatic" }, { value: 1 / 3, label: "One third" }, { value: 1 / 2, label: "Half" }, { value: 2 / 3, label: "Two thirds" }, { value: 1, label: "Full" }];

const KEY = "cal.canvas.prefs";
class CanvasPrefs {
  /** Whether the shell shows windows on a canvas or one app in a single card. The default is the build's: `VITE_CANVAS_DEFAULT=1` makes the canvas the default (the e2e gate runs with it on and off); it flips to on by default when every suite passes with it on. */
  enabled = $state(import.meta.env.VITE_CANVAS_DEFAULT === "1");
  /** How every window looks. */
  look = $state<Look>("card");
  /** The share of the canvas a new window opens at, or null for automatic. */
  width = $state<number | null>(null);
  constructor() {
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "null") as { look?: Look; width?: number | null; enabled?: boolean } | null;
      if (typeof s?.enabled === "boolean") this.enabled = s.enabled;
      if (s?.look && LOOKS.some((l) => l.value === s.look)) this.look = s.look;
      if (s && (s.width === null || OPEN_WIDTHS.some((w) => w.value === s.width))) this.width = s.width ?? null;
    } catch { /* nothing kept */ }
  }
  private save() { try { localStorage.setItem(KEY, JSON.stringify({ look: this.look, width: this.width, enabled: this.enabled })); } catch { /* storage is off */ } }
  /** Turns the canvas on or off (off: one app in a single card). */
  setEnabled(on: boolean): void { this.enabled = on; this.save(); }
  /** Chooses how every window looks. */
  setLook(look: Look): void { this.look = look; this.save(); }
  /** Chooses how wide a new window opens (null: automatic). */
  setWidth(width: number | null): void { this.width = width; this.save(); }
}
/** The canvas's preferences (see the file's comment). */
export const canvasPrefs = new CanvasPrefs();
