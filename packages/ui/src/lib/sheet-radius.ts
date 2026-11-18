/**
 * The corners of an inset sheet on an iPhone (ported from the first app, measured on Safari's share sheet in the iPhone 18 Pro simulator, iOS 27, 2026-10-20: 8 pt in from the sides, 9 pt from the bottom, top corners
 * about 40 pt, bottom about 53 pt). An iOS sheet is not one radius all round: the bottom is concentric with the phone's own display corners (the display's radius minus how far the sheet sits in from the edge),
 * the top is smaller. The web has no API for the display's corner radius, so it is looked up from the screen's logical size (portrait or landscape) for the iPhones that exist; an unknown one gets the middle value.
 * Everywhere else (a desktop, a tablet, an Android) the sheets keep the pane radius all round (`--r-pane`, the tokens' default for `--sheet-r-top` and `--sheet-r-bottom`).
 *
 * Sets `--display-radius`, `--sheet-r-top` and `--sheet-r-bottom` on the root (every bottom sheet draws them through `--sheet-corners`) and `data-sheet-r="device"`.
 */

/** Logical screen size (the short side by the long side) to the display's corner radius in points. */
const DISPLAY_RADIUS: [number, number, number][] = [
  [375, 812, 44], // X, XS, 11 Pro (39), 12 and 13 mini (44)
  [390, 844, 47], // 12, 13, 14
  [393, 852, 55], // 14 Pro, 15, 15 Pro, 16, 16 Pro
  [402, 874, 61], // 16 Pro, 17, 17 Pro (6.3"): measured on Safari's share sheet in the iPhone 18 Pro simulator
  [414, 896, 41.5], // XR, 11
  [420, 912, 55], // Air
  [428, 926, 53], // 12 and 13 Pro Max, 14 Plus
  [430, 932, 55], // 14 Pro Max, 15 Plus, 15 Pro Max, 16 Plus
  [440, 956, 55], // 16 Pro Max, 17 Pro Max
];
const FALLBACK = 47;
/** How far an inset sheet sits in from the screen's side (the token `--sheet-inset`). */
const INSET = 8;
/** The top radius, the same on every phone. */
const TOP = 40;

const isPhone = (): boolean => typeof CSS !== "undefined" && CSS.supports("-webkit-touch-callout", "none") && matchMedia("(pointer: coarse)").matches && Math.min(screen.width, screen.height) <= 440;

/** The display's corner radius in points for this screen (the nearest known iPhone; 47 when the size is not one). */
export function displayRadius(): number {
  const a = Math.min(screen.width, screen.height);
  const b = Math.max(screen.width, screen.height);
  const hit = DISPLAY_RADIUS.find(([w, h]) => Math.abs(w - a) <= 1 && Math.abs(h - b) <= 1);
  return hit ? hit[2] : FALLBACK;
}

/** Turns the iPhone sheet corners on, on an iPhone or when `force` is true (a preview or test switch); returns the way to undo it. Cheap: three custom properties on the root, once. */
export function applySheetRadius(force = false): () => void {
  const root = document.documentElement;
  if (!force && !isPhone()) return () => {};
  const d = displayRadius();
  root.style.setProperty("--display-radius", `${d}px`);
  root.style.setProperty("--sheet-r-top", `${TOP}px`);
  root.style.setProperty("--sheet-r-bottom", `${Math.max(TOP, Math.round(d - INSET))}px`);
  root.dataset.sheetR = "device";
  return () => {
    root.style.removeProperty("--display-radius");
    root.style.removeProperty("--sheet-r-top");
    root.style.removeProperty("--sheet-r-bottom");
    delete root.dataset.sheetR;
  };
}
