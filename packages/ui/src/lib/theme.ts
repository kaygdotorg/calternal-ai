/** Colour scheme and device form: the two facts every surface adapts to. Both live on <html> (`data-scheme`, CSS media queries) so CSS needs no script. */

/** The scheme actually drawn. */
export type Scheme = "light" | "dark";
/** What the person chose: follow the device, or force one. */
export type SchemePreference = "system" | Scheme;
/** Sizes, spacing and blur are tuned separately for these (tokens.json `desktop` and `touch`). */
export type Form = "desktop" | "touch";

const KEY = "cal.scheme";
const listeners = new Set<(s: Scheme) => void>();

const dark = (): MediaQueryList => matchMedia("(prefers-color-scheme: dark)");
const desktop = (): MediaQueryList => matchMedia("(min-width: 760px) and (hover: hover) and (pointer: fine)");

/** The saved preference, or "system". Costs one localStorage read. */
export function schemePreference(): SchemePreference {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

/** The scheme a preference resolves to right now. */
export function resolveScheme(pref: SchemePreference = schemePreference()): Scheme {
  return pref === "system" ? (dark().matches ? "dark" : "light") : pref;
}

/** The scheme currently on <html>. */
export function currentScheme(): Scheme {
  return document.documentElement.dataset.scheme === "dark" ? "dark" : "light";
}

/** Whether the current device gets the desktop tuning or the touch one (the same query the generated CSS uses). */
export function currentForm(): Form {
  return desktop().matches ? "desktop" : "touch";
}

/** Applies a preference: sets `data-scheme`, saves it and tells subscribers (the material re-samples). */
export function applyScheme(pref: SchemePreference): Scheme {
  const s = resolveScheme(pref);
  document.documentElement.dataset.scheme = s;
  try {
    if (pref === "system") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, pref);
  } catch {
    /* private mode: the choice lasts for this page only */
  }
  for (const fn of listeners) fn(s);
  return s;
}

/** Applies the saved preference and follows the device while the preference is "system". Call once at start. */
export function initScheme(): void {
  applyScheme(schemePreference());
  dark().addEventListener("change", () => {
    if (schemePreference() === "system") applyScheme("system");
  });
}

/** Subscribes to scheme changes; returns the unsubscribe. */
export function onScheme(fn: (s: Scheme) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
