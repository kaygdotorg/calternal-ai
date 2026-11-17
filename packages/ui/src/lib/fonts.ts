/** The text font follows the scheme (tokens: "Google Sans Flex" light, "Google Sans Flex Dark" dark; same advance widths). Only the active family loads. */
import type { Scheme } from "./theme";

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
