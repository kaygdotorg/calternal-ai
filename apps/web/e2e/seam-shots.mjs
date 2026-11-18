// The four corners of the middle card, zoomed, light and dark, at 2x on the default wallpaper (SHOTS=<dir>): a picture of what a person sees where the card's rounded corners meet the background. Each corner is a 64 CSS px square
// around the corner of the card's box, enlarged 6x without smoothing, with the real content in place; plus the whole window. For the issue, not for the gate.
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const dir = process.env.SHOTS ?? "/tmp/seam-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
for (const scheme of ["light", "dark"]) {
  const ctx = await b.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage(); await page.goto(`${BASE}/threads/t-run?panel=changes`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2000);
  const g = await page.evaluate(() => { const r = document.querySelector(".mid-card").getBoundingClientRect(); return { x0: r.left, x1: r.right, y0: r.top, y1: r.bottom }; });
  await page.screenshot({ path: `${dir}/window-${scheme}.png` });
  const shot = (await page.screenshot()).toString("base64");
  const sheet = await page.evaluate(async ([png, g]) => {
    const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
    const S = 64, Z = 6, d = img.width / 1500, out = document.createElement("canvas"); out.width = S * Z * 2 + 8; out.height = S * Z * 2 + 8; const x = out.getContext("2d"); x.imageSmoothingEnabled = false; x.fillStyle = "#f0f"; x.fillRect(0, 0, out.width, out.height);
    [[g.x0, g.y0], [g.x1, g.y0], [g.x0, g.y1], [g.x1, g.y1]].forEach(([cx, cy], k) => { const sx = Math.min(img.width - S * d, Math.max(0, (cx - S / 2) * d)), sy = Math.min(img.height - S * d, Math.max(0, (cy - S / 2) * d)); x.drawImage(img, sx, sy, S * d, S * d, (k % 2) * (S * Z + 8), Math.floor(k / 2) * (S * Z + 8), S * Z, S * Z); });
    return out.toDataURL("image/png").split(",")[1];
  }, [shot, g]);
  (await import("node:fs")).writeFileSync(`${dir}/corners-${scheme}.png`, Buffer.from(sheet, "base64"));
  await ctx.close();
}
await b.close(); stop();
