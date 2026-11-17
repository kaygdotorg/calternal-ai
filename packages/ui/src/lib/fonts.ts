/** The text font follows the scheme (tokens: "Google Sans Flex" light, "Google Sans Flex Dark" dark; same advance widths). Only the active family loads. */
import type { Scheme } from "./theme";

/** One font file the app ships (copied by apps/web/scripts/fonts.mjs). `range` limits a subset file to its characters. */
interface Face {
  family: string;
  file: string;
  weight: string;
  range?: string;
  display: "block";
  mono?: boolean;
}

const LATIN = "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD";
const EXT = "U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+0304, U+0308, U+0329, U+1D00-1DBF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C0, U+2113, U+2C60-2C7F, U+A720-A7FF";
const SYMBOLS = "U+2190-21FF, U+2300-23FF, U+2460-24FF, U+25A0-27BF";

/**
 * The shipped faces. Google Sans Flex (light: weight + optical size; Dark: the same plus GRAD for more ink with identical advance widths;
 * Rounded: weight + ROND, used only where the existing app uses it, with `font-variation-settings: "ROND" 100`) and Maple Mono.
 */
const FACES: Face[] = [
  { family: "Google Sans Flex", file: "google-sans-flex-latin-opsz-normal.woff2", weight: "1 1000", range: LATIN, display: "block" },
  { family: "Google Sans Flex", file: "google-sans-flex-latin-ext-opsz-normal.woff2", weight: "1 1000", range: EXT, display: "block" },
  { family: "Google Sans Flex", file: "google-sans-flex-symbols-opsz-normal.woff2", weight: "1 1000", range: SYMBOLS, display: "block" },
  { family: "Google Sans Flex Dark", file: "google-sans-flex-latin-opsz-grad-normal.woff2", weight: "300 800", range: LATIN, display: "block" },
  { family: "Google Sans Flex Dark", file: "google-sans-flex-latin-ext-opsz-grad-normal.woff2", weight: "300 800", range: EXT, display: "block" },
  { family: "Google Sans Flex Dark", file: "google-sans-flex-symbols-opsz-normal.woff2", weight: "1 1000", range: SYMBOLS, display: "block" },
  { family: "Google Sans Flex Rounded", file: "google-sans-flex-latin-rond-normal.woff2", weight: "1 1000", display: "block" },
  { family: "Dancing Script", file: "dancing-script-latin-wght-normal.woff2", weight: "400 700", display: "block" },
  { family: "Maple Mono", file: "maple-mono-latin-400-normal.woff2", weight: "400", display: "block", mono: true },
  { family: "Maple Mono", file: "maple-mono-latin-500-normal.woff2", weight: "500", display: "block", mono: true },
];

/** Registers the shipped faces under `base` (the app's mount path, ending in a slash). Nothing downloads until a face is used (or `loadFonts` asks), so only the active scheme's family is ever fetched. */
export function registerFonts(base: string): void {
  for (const f of FACES) {
    const src = `url(${base}fonts/${f.file}) format("${f.mono ? "woff2" : "woff2-variations"}")`;
    const face = new FontFace(f.family, src, { weight: f.weight, display: f.display, ...(f.range ? { unicodeRange: f.range } : {}) });
    document.fonts.add(face);
  }
}

/** CSS family name for a scheme. */
export const familyFor = (s: Scheme): string => (s === "dark" ? "Google Sans Flex Dark" : "Google Sans Flex");
const faces = (s: Scheme) => [400, 650].map((w) => `${w} 17px "${familyFor(s)}"`);

/** Starts loading the scheme's family (the two weights the first screen uses); resolves when it is in. */
export const loadFonts = (s: Scheme): Promise<unknown> => Promise.all(faces(s).map((f) => document.fonts.load(f)));

/** Null when the scheme's family is already loaded; otherwise a promise that settles when it is or after `ms`, so a slow network never blocks a switch. */
export function fontsReady(s: Scheme, ms = 700): Promise<unknown> | null {
  if (faces(s).every((f) => document.fonts.check(f))) return null;
  return Promise.race([loadFonts(s).catch(() => {}), new Promise((r) => setTimeout(r, ms))]);
}
