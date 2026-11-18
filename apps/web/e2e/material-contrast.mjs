// How different the middle card and the sidebars look: the contrast ratio (WCAG, 1 to 21) between the mean colour of the card and of each sidebar with all text, icons and rules hidden, on the default
// wallpaper, light and dark, desktop 1500x970. The owner's complaint ("the card and the sidebars look a lot like each other") is a ratio near 1. Prints one line per scheme and exits 0 (a measurement).
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const rows = [];
for (const scheme of ["dark", "light"]) {
  const ctx = await b.newContext({ viewport: { width: 1500, height: 970 }, colorScheme: scheme, deviceScaleFactor: 1 });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  if (process.env.WALL) await ctx.addInitScript((w) => localStorage.setItem("cal.appearance", JSON.stringify({ wallpaperId: w })), process.env.WALL);
  const page = await ctx.newPage();
  await page.goto(`${BASE}/threads/t-idle`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--c-dim-a").trim() !== "", null, { timeout: 15000 }).catch(() => {});
  await page.addStyleTag({ content: "*, *::before:not(.keep), *::after:not(.keep) { color: transparent !important; border-color: transparent !important; box-shadow: none !important; text-shadow: none !important; } svg, img { visibility: hidden !important; } aside .row, aside button, .composer, .chips, header { visibility: hidden !important; }" });
  await page.waitForTimeout(800);
  const rects = await page.evaluate(() => { const r = (s) => { const e = document.querySelector(s); const b = e?.getBoundingClientRect(); return b ? { x: b.x, y: b.y, w: b.width, h: b.height } : null; }; return { left: r("aside.side"), card: r(".mid-card"), right: r("aside.right, .right") }; });
  const shot = (await page.screenshot()).toString("base64");
  const res = await page.evaluate(async ([png, rects]) => {
    const img = await createImageBitmap(await (await fetch(`data:image/png;base64,${png}`)).blob());
    const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const g = c.getContext("2d", { willReadFrequently: true }); g.drawImage(img, 0, 0);
    const lin = (v) => ((v /= 255) <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
    const mean = (r) => { if (!r) return null; const x = Math.round(r.x + r.w * 0.1), y = Math.round(r.y + r.h * 0.1), w = Math.max(1, Math.round(r.w * 0.8)), h = Math.max(1, Math.round(r.h * 0.8)); const d = g.getImageData(x, y, w, h).data; let R = 0, G = 0, B = 0, n = 0; for (let i = 0; i < d.length; i += 4) { R += d[i]; G += d[i + 1]; B += d[i + 2]; n++; } const [r8, g8, b8] = [R / n, G / n, B / n]; return { rgb: [Math.round(r8), Math.round(g8), Math.round(b8)], L: 0.2126 * lin(r8) + 0.7152 * lin(g8) + 0.0722 * lin(b8) }; };
    return { left: mean(rects.left), card: mean(rects.card), right: mean(rects.right) };
  }, [shot, rects]);
  const ratio = (a, c) => (Math.max(a, c) + 0.05) / (Math.min(a, c) + 0.05);
  rows.push({ scheme, left: res.left?.rgb, card: res.card?.rgb, right: res.right?.rgb, cardVsLeft: res.left && res.card ? +ratio(res.left.L, res.card.L).toFixed(2) : null, cardVsRight: res.right && res.card ? +ratio(res.right.L, res.card.L).toFixed(2) : null });
  await ctx.close();
}
console.log(JSON.stringify(rows, null, 1));
await b.close(); stop();
