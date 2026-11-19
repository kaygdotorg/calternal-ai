import { appearance } from "../../appearance/prefs.svelte";
import { accentOf } from "./accent";
import type { ChromeAppearance } from "./types";

const root = () => document.documentElement.style;
const rgb = (name: string): [number, number, number] | null => { const m = /^\s*(\d+)\s+(\d+)\s+(\d+)\s*$/.exec(root().getPropertyValue(name)); return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null; };
const num = (name: string): number => { const v = parseFloat(root().getPropertyValue(name)); return Number.isFinite(v) ? v : 0; };
const layer = (k: "l" | "c", what: "tint" | "dim"): [number, number, number, number] | null => { const c = rgb(`--${k}-${what}`); return c ? [...c, num(`--${k}-${what}-a`)] as [number, number, number, number] : null; };
const ink = (k: "l" | "c", n: 2 | 3): string | undefined => root().getPropertyValue(`--${k}-ink${n}`).trim() || undefined;

/**
 * The person's look as the native shell needs it to paint the wallpaper and the glass itself: the wallpaper (a library id, `auto` or none, and its address when it is in the built-in library), the Blur, Dim and Tint
 * switches, Clear to tinted, the scheme, the accent and the interface scale, and the veils the page has solved from the picture (the sidebars' as `side`, the card's as `card`: tint and dim as `[r, g, b, a]`, and the two
 * quieter inks), so the shell does not solve them again. A custom or searched wallpaper has no address the shell can fetch (it lives in the page's own storage): the shell then gets the id only. Reads reactive preferences
 * (so a change republishes) and the root's solved variables (read when the description is built).
 */
export function appearanceOf(scheme: "light" | "dark"): ChromeAppearance {
  const p = appearance.prefs;
  const id = p.wallpaperId;
  const builtIn = id && id !== "auto" && /^[a-z0-9-]+$/i.test(id);
  const side = { tint: layer("l", "tint"), dim: layer("l", "dim") }, card = { tint: layer("c", "tint"), dim: layer("c", "dim") };
  const veils: ChromeAppearance["veils"] = side.tint && side.dim && card.tint && card.dim
    ? { side: { tint: side.tint, dim: side.dim, ink2: ink("l", 2), ink3: ink("l", 3) }, card: { tint: card.tint, dim: card.dim, ink2: ink("c", 2), ink3: ink("c", 3) } }
    : undefined;
  return {
    wallpaper: { id, ...(builtIn ? { url: `${import.meta.env.BASE_URL}wallpapers/${id}.jpg` } : {}) },
    blur: p.materials.blur, dim: p.materials.dim, tint: p.materials.tint, clarity: p.clarity, scheme, accent: accentOf(), uiScale: p.uiSize / 100,
    ...(veils ? { veils } : {}),
  };
}
