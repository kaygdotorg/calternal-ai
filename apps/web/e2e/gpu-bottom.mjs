// The bottom edge in a GPU-backed Chrome at 2x: row-by-row mean brightness of the last rows beside the right sidebar's tab capsule and under the card, for a few window sizes. Usage: BASE=... node gpu-bottom.mjs
import { chromium } from "playwright-core";
const BASE = process.env.BASE;
const b = await chromium.connectOverCDP("http://127.0.0.1:9222");
const ctx = b.contexts()[0] ?? await b.newContext();
for (const [w, h] of [[1500, 970], [1493, 967], [1512, 945]]) {
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 2, mobile: false });
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2200);
  const shot = await page.screenshot({ clip: { x: 0, y: h - 60, width: w, height: 60 }, scale: "device" });
  const r = await page.evaluate(async (b64) => {
    const img = new Image(); img.src = "data:image/png;base64," + b64; await img.decode();
    const cv = document.createElement("canvas"); cv.width = img.width; cv.height = img.height; const c = cv.getContext("2d", { willReadFrequently: true }); c.drawImage(img, 0, 0);
    const strip = (x0, x1) => { const d = c.getImageData(x0, 0, x1 - x0, img.height).data, w = x1 - x0, m = []; for (let y = 0; y < img.height; y++) { let a = 0; for (let x = 0; x < w; x++) { const i = (y * w + x) * 4; a += (d[i] + d[i + 1] + d[i + 2]) / 3; } m.push(a / w); } let worst = 0, at = 0; for (let y = 1; y < m.length; y++) if (Math.abs(m[y] - m[y - 1]) > worst) { worst = Math.abs(m[y] - m[y - 1]); at = y; } return { worst: +worst.toFixed(2), at, last: +m.at(-1).toFixed(1), tail: m.slice(-8).map((v) => +v.toFixed(1)) }; };
    return { sidebar: strip(img.width - 330 * 2, img.width - 20 * 2), card: strip(600 * 2, 900 * 2), px: [img.width, img.height] };
  }, shot.toString("base64"));
  console.log(`${w}x${h}`, JSON.stringify(r));
  await page.close();
}
await b.close();
