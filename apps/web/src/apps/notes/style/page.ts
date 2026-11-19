/**
 * The note's page style, read from its frontmatter (`calternal-*` keys): pure, no editor and no DOM. Ported from the existing app's `lib/page-style.ts` (origin/dev), keeping its key names and
 * its rules (a phone's page is always full width; an unknown value reads as the default and is left in the file untouched). Differences are listed in docs/notes-style.md.
 */

/** How wide the note sits: `narrow` (the default), `wide`, or `full` (the card's whole width). */
export type Width = "narrow" | "wide" | "full";
/** The font choices: `system` is the editor's own (chat) font; `mono` and `rounded` use the code and rounded roles. */
export type Font = "system" | "mono" | "rounded";
/** The note's page as the Style sheet and the card use it. */
export interface PageStyle {
  font: Font;
  width: Width;
  /** The page's opacity, 0 to 1; 1 is solid paper. Null follows the app (no page surface). */
  opacity: number | null;
  /** The page's backdrop blur in px; null follows the app. */
  blur: number | null;
  /** The note's tree lines (`dotted` or `connected`), or null to follow the editor default. */
  lines: "dotted" | "connected" | null;
  /** The border around the page (`calternal-frame: off` removes it). */
  frame: boolean;
  /** The note's text size. */
  size: "small" | "default" | "large";
  /** The page colour by name, or "default". */
  colour: string;
  /** A theme id (a preset's palette), or "" for the app's. */
  theme: string;
  /** Paper grain on a frosted page. */
  grain: boolean;
  /** The note's backdrop id, or null to follow the app. */
  backdrop: string | null;
}

/** The page colours a note can choose (a tint mixed into the theme's paper, so they work in dark too); `default` writes no line. */
export const PAPERS: readonly { id: string; label: string; hue: string | null }[] = [
  { id: "default", label: "Paper", hue: null },
  { id: "sand", label: "Sand", hue: "#d9a441" },
  { id: "sage", label: "Sage", hue: "#6f9a5a" },
  { id: "sky", label: "Sky", hue: "#4f8fc0" },
  { id: "rose", label: "Rose", hue: "#c9667a" },
  { id: "slate", label: "Slate", hue: "#6a6f86" },
];

/** The page fill for a stored colour name, or undefined for the default or an unknown name. */
export const paperOf = (id: string): string | undefined => {
  const hue = PAPERS.find((p) => p.id === id)?.hue;
  return hue ? `color-mix(in oklab, var(--paper) 78%, ${hue})` : undefined;
};

const percent = (v: string | undefined): number | null => {
  if (!v) return null;
  const n = parseFloat(v) / 100;
  return Number.isFinite(n) ? Math.max(0, Math.min(1, n)) : null;
};

/** The width a note asks for. `phone` is true on a phone, where the page is always the card's width; the file keeps its line. */
export function widthOf(fm: Record<string, string>, phone = false): Width {
  if (phone) return "full";
  const w = fm["calternal-width"];
  if (w === "narrow" || w === "wide" || w === "full") return w;
  return "narrow";
}

/** Reads the page style from the frontmatter pairs the editor reports. */
export function pageStyle(fm: Record<string, string>, phone = false): PageStyle {
  const font = fm["calternal-font"];
  const blur = fm["calternal-page-blur"] ? parseFloat(fm["calternal-page-blur"]) : NaN;
  return {
    font: font === "mono" || font === "rounded" ? font : "system",
    width: widthOf(fm, phone),
    opacity: percent(fm["calternal-page-opacity"]),
    blur: Number.isFinite(blur) ? Math.max(0, Math.min(60, blur)) : null,
    lines: fm["calternal-lines"] === "dotted" || fm["calternal-lines"] === "connected" ? fm["calternal-lines"] : null,
    frame: fm["calternal-frame"] !== "off",
    size: fm["calternal-text-size"] === "small" || fm["calternal-text-size"] === "large" ? fm["calternal-text-size"] : "default",
    colour: PAPERS.some((p) => p.id === fm["calternal-page-colour"]) ? fm["calternal-page-colour"]! : "default",
    theme: /^[a-z][a-z-]{0,30}$/.test(fm["calternal-theme"] ?? "") ? fm["calternal-theme"]! : "",
    grain: fm["calternal-grain"] === "on",
    backdrop: fm["calternal-backdrop"] || null,
  };
}
