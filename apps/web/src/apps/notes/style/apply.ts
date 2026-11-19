/**
 * Drawing a note's style on its card, cheaply: the page style becomes attributes and a few custom properties on the card (CSS does the rest), and a note's theme becomes the preset's palette tokens
 * set on that card only (the app is untouched). Nothing here runs per keystroke: it runs when the frontmatter changes or the colour scheme flips. Themes are the app's own presets with a palette
 * (appearance/presets.ts); the existing app's other palettes are listed as a gap in docs/notes-style.md.
 */
import { onScheme, currentScheme } from "@calternal/ui";
import { PRESETS } from "../../../appearance/presets";
import { tokensFor } from "../../../appearance/theme";
import { paperOf, type PageStyle } from "./page";

/** The themes a note can choose: the presets that carry a palette, as `{ id, name }`. "Same as app" is no theme and writes no line. */
export const THEMES: readonly { id: string; name: string }[] = PRESETS.filter((p) => p.theme).map((p) => ({ id: p.id, name: p.name }));

const VARS = ["paper", "tint", "ink", "ink-rgb", "shadow-rgb", "ink-2", "ink-3", "ink-4", "accent-base", "accent", "accent-soft", "danger", "good", "warn", "info", "accent-fill-ink"];

/** Sets (or clears) the palette tokens of a theme for a scheme on one element. */
export function setThemeVars(el: HTMLElement, themeId: string, scheme: "light" | "dark"): void {
  for (const k of VARS) el.style.removeProperty(`--${k}`);
  const theme = PRESETS.find((p) => p.id === themeId)?.theme;
  if (!theme) return;
  for (const [k, v] of Object.entries(tokensFor(theme[scheme], scheme))) el.style.setProperty(`--${k}`, v);
}

/** Svelte action: keeps the note card's theme tokens current for the chosen theme and the colour scheme. */
export function pageTheme(el: HTMLElement, themeId: string): { update(id: string): void; destroy(): void } {
  let id = themeId;
  const draw = () => setThemeVars(el, id, currentScheme());
  draw();
  const stop = onScheme(draw);
  return { update(next) { id = next; draw(); }, destroy() { stop(); setThemeVars(el, "", "light"); } };
}

/** The inline custom properties for a page style: page opacity, blur and colour. Only the keys a note has produce anything, so a plain note carries no style at all. */
export function pageVars(p: PageStyle): string {
  const out: string[] = [];
  if (p.opacity !== null) out.push(`--page-opacity:${p.opacity}`);
  if (p.blur !== null) out.push(`--page-blur:${p.blur}px`);
  const fill = paperOf(p.colour);
  if (fill) out.push(`--page-fill:${fill}`);
  return out.join(";");
}

/** Whether the page draws as a surface (a card) rather than as the bare column: any of opacity, blur, colour or a frame the note asked for. */
export const hasSurface = (p: PageStyle, fm: Record<string, string>): boolean => p.opacity !== null || p.blur !== null || p.colour !== "default" || "calternal-frame" in fm || p.grain || p.theme !== "";
