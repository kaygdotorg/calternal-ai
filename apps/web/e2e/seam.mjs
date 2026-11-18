// One continuous background (docs/ui-shell.md, "The background"): the wallpaper under ONE veil spans the whole window and both sidebars are drawn on it; the middle card is the only thing on top. Nothing may paint in the
// gap beside the card or behind its rounded corners. Measured on real pixels: text, icons and the sidebars' own content are hidden, then the four corner regions OUTSIDE the card's radius (and the gap columns beside the
// card, and the strips above and below it) are compared with the surface right next to them, across the line where a second layer would begin. A seam is a step in colour at that line; a smooth wallpaper changes slowly, so
// the steps allowed are a few levels out of 255. Both schemes, the eight wallpapers, at 1x and 2x (the gate runs fewer: SEAM_FULL=1 for all).
import { chromium } from "playwright";
import { readFileSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const IDS = JSON.parse(readFileSync("wallpapers.json", "utf8")).map((w) => w.id);
const FULL = !!process.env.SEAM_FULL, STEP = Number(process.env.SEAM_MAX ?? 4);
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
    const px = (cx, cy) => x.getImageData(Math.round(cx * dpr), Math.round(cy * dpr), 1, 1).data;
    const d = (a, b) => Math.max(Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1]), Math.abs(a[2] - b[2]));
    const R = Math.ceil(g.radius), out = [];
    const outside = (dx, dy) => Math.hypot(R - dx, R - dy) > R + 0.5; // (dx, dy) measured from the corner of the card's box: outside the rounded corner when farther than R from the arc's centre
    // For each corner: rows (or columns) crossing the line that the card's edge makes: compare a pixel just outside the card with the pixel it would continue into, 3 px farther out, along the same row.
    const corners = [["top-left", g.x0, g.y0, 1, 1], ["top-right", g.x1, g.y0, -1, 1], ["bottom-left", g.x0, g.y1, 1, -1], ["bottom-right", g.x1, g.y1, -1, -1]];
    for (const [name, cx, cy, sx, sy] of corners) {
      let worst = 0;
      for (let k = 0; k <= R; k++) {
        // horizontal line k px from the card's top (or bottom) edge: the first pixel outside the rounded corner, and the one 4 px beyond it away from the card
        const dy = k; let dx = 0; while (dx <= R && !outside(dx, R - dy ? R - (R - dy) + 0 : 0) && dx < R) dx++;
        const yy = cy + sy * (k + 0.5);
        // inside the corner square, the card's edge on this row is at xEdge = cx + sx * (R - sqrt(R^2 - (R - k)^2))
        const t = R - k, xe = cx + sx * (R - Math.sqrt(Math.max(0, R * R - t * t)));
        const a = px(xe - sx * 1.5, yy), bpx = px(xe - sx * 6, yy);
        worst = Math.max(worst, d(a, bpx));
      }
      out.push([name, worst]);
    }
    // The gap columns: beside the card's left and right edges, at several heights: the column of the gap against the column 8 px beyond it
    for (const [name, xe, s] of [["left gap", g.x0, 1], ["right gap", g.x1, -1]]) { let worst = 0; for (const f of [0.2, 0.4, 0.6, 0.8]) { const yy = g.y0 + (g.y1 - g.y0) * f; worst = Math.max(worst, d(px(xe - s * 2, yy), px(xe - s * 10, yy))); } out.push([name, worst]); }
    return out;
  }, [png, g, dpr]);
  const worst = Math.max(...res.map((r) => r[1])); worstAll = Math.max(worstAll, worst);
  const fail = res.filter((r) => r[1] > STEP);
  console.log(`${fail.length ? "FAIL" : "PASS"}  ${scheme} ${id.slice(0, 6)} ${dpr}x  steps: ${res.map((r) => `${r[0]} ${r[1]}`).join(", ")}`);
  if (fail.length) bad++;
  await ctx.close();
}
console.log(`worst step across the window's seams: ${worstAll} levels of 255 (allowed ${STEP})`);
await b.close(); stop(); process.exit(bad ? 1 : 0);
