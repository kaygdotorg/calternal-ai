// One continuous background (docs/ui-shell.md, "The background"): the wallpaper under ONE veil spans the whole window and both sidebars are drawn on it; the middle card is the only thing on top. Nothing may paint in the
// gap beside the card or behind its rounded corners. Measured on real pixels: text, icons and the sidebars' own content are hidden, then the four corner regions OUTSIDE the card's radius (and the gap columns beside the
// card, and the strips above and below it) are compared with the surface right next to them, across the line where a second layer would begin. A seam is a step in colour at that line larger than the wallpaper's own change over the same distance (measured well inside the sidebars as a control) by more than 3 levels of 255. Both schemes, the eight wallpapers, at 1x and 2x (the gate runs fewer: SEAM_FULL=1 for all).
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const IDS = JSON.parse(readFileSync("wallpapers.json", "utf8")).map((w) => w.id);
const FULL = !!process.env.SEAM_FULL, STEP = Number(process.env.SEAM_MAX ?? 3);
const stop = await serve(); const b = await chromium.launch(); let bad = 0, worstAll = 0;
const cases = [];
for (const scheme of ["light", "dark"]) for (const id of FULL ? IDS : [IDS[0], IDS[3]]) for (const dpr of FULL ? [1, 2] : id === IDS[0] ? [1, 2] : [1]) cases.push({ scheme, id, dpr });
for (const { scheme, id, dpr } of cases) {
  const ctx = await b.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: dpr, colorScheme: scheme });
  await ctx.addInitScript(([id]) => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: id })); }, [id]);
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run?panel=changes`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "" && document.documentElement.style.getPropertyValue("--wall-side").includes("blob"), null, { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(1200);
  await page.addStyleTag({ content: "aside > *, .edge, .mid-card > * { visibility: hidden !important; } .mid-card { box-shadow: none !important; } *, *::before, *::after { caret-color: transparent !important; }" });
  await page.waitForTimeout(300);
  const g = await page.evaluate(() => { const c = document.querySelector(".mid-card"), r = c.getBoundingClientRect(); return { x0: r.left, x1: r.right, y0: r.top, y1: r.bottom, radius: parseFloat(getComputedStyle(c).borderTopLeftRadius) || 0, iw: innerWidth, ih: innerHeight }; });
  const png = (await page.screenshot()).toString("base64");
  const res = await page.evaluate(async ([png, g, dpr]) => {
    const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
    const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const x = c.getContext("2d", { willReadFrequently: true }); x.drawImage(img, 0, 0);
    // The mean colour of a 5 x 5 device-pixel patch (the grain is noise of a few levels per pixel; a seam is a step in the mean).
    const at = (cx, cy) => { const px = Math.round(cx * dpr) - 2, py = Math.round(cy * dpr) - 2; if (px < 0 || py < 0 || px + 5 > img.width || py + 5 > img.height) return null; const d = x.getImageData(px, py, 5, 5).data; let r = 0, g2 = 0, b = 0; for (let i = 0; i < d.length; i += 4) { r += d[i]; g2 += d[i + 1]; b += d[i + 2]; } return [r / 25, g2 / 25, b / 25]; };
    // A seam is a step, not a slope: the value 3 px outside the card is compared with what the two rings farther out (12 and 21 px) predict for it by a straight line. A smooth gradient (the photograph) is on that line; a layer that begins at the card's edge is not.
    const resid = (a, b, c) => Math.max(...[0, 1, 2].map((i) => Math.abs(a[i] - (2 * b[i] - c[i]))));
    const R = g.radius, out = [];
    // Around each rounded corner, leaving the card's edge outward along the radius: the ring 3 px outside the card against the ring 12 px outside it. Where a second layer began at the card's edge (a veil that stops, a shadow, a
    // backdrop of the card's own) the two rings differ by a step; on one continuous background they differ by the wallpaper's slow gradient.
    const corners = [["top-left", g.x0 + R, g.y0 + R, Math.PI, 1.5 * Math.PI], ["top-right", g.x1 - R, g.y0 + R, 1.5 * Math.PI, 2 * Math.PI], ["bottom-right", g.x1 - R, g.y1 - R, 0, 0.5 * Math.PI], ["bottom-left", g.x0 + R, g.y1 - R, 0.5 * Math.PI, Math.PI]];
    for (const [name, cx, cy, a0, a1] of corners) {
      let worst = 0;
      for (let k = 0; k <= 12; k++) { const t = a0 + ((a1 - a0) * k) / 12, ux = Math.cos(t), uy = Math.sin(t); const a = at(cx + ux * (R + 3), cy + uy * (R + 3)), b2 = at(cx + ux * (R + 12), cy + uy * (R + 12)), c3 = at(cx + ux * (R + 21), cy + uy * (R + 21)); if (a && b2 && c3) worst = Math.max(worst, resid(a, b2, c3)); }
      out.push([name, +worst.toFixed(1)]);
    }
    // The gap columns beside the card at several heights, and the strips above and below it where the card does not reach the window's edge.
    for (const [name, xe, s2] of [["left gap", g.x0, -1], ["right gap", g.x1, 1]]) { let worst = 0; for (const f of [0.15, 0.3, 0.5, 0.7, 0.85]) { const yy = g.y0 + (g.y1 - g.y0) * f; const a = at(xe + s2 * 3, yy), b2 = at(xe + s2 * 12, yy), c3 = at(xe + s2 * 21, yy); if (a && b2 && c3) worst = Math.max(worst, resid(a, b2, c3)); } out.push([name, +worst.toFixed(1)]); }
    // The control: how much the wallpaper itself changes over the same 9 px well inside each sidebar (nothing begins or ends there). A seam is a step larger than that.
    let natural = 0;
    for (const xs of [40, 120, g.x1 + 40, g.x1 + 120]) for (const f of [0.2, 0.4, 0.6, 0.8]) { const yy = g.y0 + (g.y1 - g.y0) * f; const a = at(xs, yy), b2 = at(xs + 9, yy), c3 = at(xs + 18, yy); if (a && b2 && c3) natural = Math.max(natural, resid(a, b2, c3)); }
    out.push(["natural", +natural.toFixed(1)]);
    return out;
  }, [png, g, dpr]);
  const natural = res.find((r) => r[0] === "natural")[1], seams = res.filter((r) => r[0] !== "natural");
  const worst = Math.max(...seams.map((r) => r[1] - natural)); worstAll = Math.max(worstAll, worst);
  const fail = seams.filter((r) => r[1] - natural > STEP);
  console.log(`${fail.length ? "FAIL" : "PASS"}  ${scheme} ${id.slice(0, 6)} ${dpr}x  steps: ${res.map((r) => `${r[0]} ${r[1]}`).join(", ")}`);
  if (fail.length) bad++;
  await ctx.close();
}
console.log(`worst excess step over the control across the window's seams: ${worstAll.toFixed(1)} levels of 255 (allowed ${STEP})`);
await b.close(); stop(); process.exit(bad ? 1 : 0);
