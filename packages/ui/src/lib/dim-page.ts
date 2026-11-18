/**
 * Dims what lies outside the page while a row's preview is up (the status bar, the strip under Safari's toolbar). The page itself is dimmed by
 * the press scrim; this is only the edge, and it is done with the two things that cost nothing to change: the body's own background colour and
 * the `theme-color` of the page (a custom property or an attribute changed on the root restyles every element of the page, several ms on a phone).
 * Ported from the existing app's dim-page.ts, without its phone-chrome switches (this app's shell sets its own).
 */
let undimmed: string | null = null;
const meta = (): HTMLMetaElement => {
  let m = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
  if (!m) { m = document.createElement("meta"); m.name = "theme-color"; document.head.append(m); }
  return m;
};

/** Dims (`true`) or restores (`false`) the page edge. Cost: one computed-style read and one 1 px canvas read, on open only. */
export function dimPage(on: boolean): void {
  const body = document.body;
  if (!on) {
    body.style.removeProperty("background-color");
    if (undimmed !== null) meta().setAttribute("content", undimmed);
    undimmed = null;
    return;
  }
  const rs = getComputedStyle(document.documentElement);
  const ink = rs.getPropertyValue("--scrim-ink").trim() || "#000";
  const amount = rs.getPropertyValue("--scrim-amount").trim() || "30%";
  const mix = `color-mix(in srgb, ${getComputedStyle(body).backgroundColor}, ${ink} ${amount})`;
  body.style.backgroundColor = mix;
  // theme-color wants a plain colour: a canvas turns the mix into pixels.
  const c = document.createElement("canvas").getContext("2d", { willReadFrequently: true })!;
  c.fillStyle = "#000"; c.fillStyle = mix; c.fillRect(0, 0, 1, 1);
  const [r, g, b] = c.getImageData(0, 0, 1, 1).data;
  undimmed ??= meta().getAttribute("content") ?? "";
  meta().setAttribute("content", `rgb(${r} ${g} ${b})`);
}
