// The four corners of the middle card in a headed, GPU-backed Chrome (what the owner uses), at 2x, light and dark, zoomed: for the issue. Run on a machine with Chrome started with --remote-debugging-port=9222:
//   BASE=https://preview.calternal.cloud/ai node seam-gpu.mjs <outDir>
// Each corner is a 64 CSS px square around the corner of the card's box, enlarged 6x without smoothing, with the real content in place; plus the whole window.
import { chromium } from "playwright-core";
import { mkdirSync, writeFileSync } from "node:fs";
const BASE = process.env.BASE, OUT = process.argv[2] ?? "/tmp/seam-gpu"; mkdirSync(OUT, { recursive: true });
const b = await chromium.connectOverCDP("http://127.0.0.1:9222");
const ctx = b.contexts()[0] ?? await b.newContext();
for (const scheme of ["light", "dark"]) {
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
  await cdp.send("Emulation.setDeviceMetricsOverride", { width: 1500, height: 970, deviceScaleFactor: 2, mobile: false });
  await cdp.send("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
  await page.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  await page.goto(`${BASE}/threads/t-run?server=0&panel=changes`); await page.waitForSelector("html[data-ready]", { timeout: 30000 }); await page.waitForTimeout(3000);
  const g = await page.evaluate(() => { const r = document.querySelector(".mid-card").getBoundingClientRect(); return { x0: r.left, x1: r.right, y0: r.top, y1: r.bottom }; });
  const full = await page.screenshot(); writeFileSync(`${OUT}/window-${scheme}.png`, full);
  const sheet = await page.evaluate(async ([png, g]) => {
    const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
    const S = 64, Z = 6, d = img.width / 1500, out = document.createElement("canvas"); out.width = S * Z * 2 + 8; out.height = S * Z * 2 + 8; const x = out.getContext("2d"); x.imageSmoothingEnabled = false; x.fillStyle = "#f0f"; x.fillRect(0, 0, out.width, out.height);
    [[g.x0, g.y0], [g.x1, g.y0], [g.x0, g.y1], [g.x1, g.y1]].forEach(([cx, cy], k) => { const sx = Math.min(img.width - S * d, Math.max(0, (cx - S / 2) * d)), sy = Math.min(img.height - S * d, Math.max(0, (cy - S / 2) * d)); x.drawImage(img, sx, sy, S * d, S * d, (k % 2) * (S * Z + 8), Math.floor(k / 2) * (S * Z + 8), S * Z, S * Z); });
    return out.toDataURL("image/png").split(",")[1];
  }, [full.toString("base64"), g]);
  writeFileSync(`${OUT}/corners-${scheme}.png`, Buffer.from(sheet, "base64"));
  await page.close();
}
await b.close();
