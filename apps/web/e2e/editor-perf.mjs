// The block editor's budgets (docs/editor-port-plan.md section 3), at 4x CPU slowdown on a fixture note of about 50,000 words: open time, keystroke to paint (the probe, ?test=1), and scroll frames.
// Usage: node e2e/editor-perf.mjs [--1x]   (after `npm run build`).
import { chromium } from "playwright";
import { serve, open, BASE, DEVICES } from "./lib.mjs";
const RATE = process.argv.includes("--1x") ? 1 : 4;
const stop = await serve(); const b = await chromium.launch();
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
const out = { cpuSlowdown: RATE };
const fresh = async (path) => {
  const ctx = await b.newContext({ ...DEVICES.desktop });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage();
  const cdp = await ctx.newCDPSession(page);
  if (RATE > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: RATE });
  await page.addInitScript(`window.__frames = []; (() => { let last = performance.now(); const f = (t) => { window.__frames.push(t - last); last = t; requestAnimationFrame(f); }; requestAnimationFrame(f); })();`);
  await page.goto(BASE + path);
  return { ctx, page, cdp };
};
// 1. Open: a short note and the long one, cold (navigation) and warm (click in the list).
{
  const { ctx, page } = await fresh("/notes/n-1?test=1");
  await page.waitForSelector(".cm-h2");
  await page.waitForTimeout(800);
  out.openShort = await page.evaluate(async () => { const t0 = performance.now(); document.querySelector('nav[aria-label="Notes"] a[href$="n-2"]').click(); await new Promise((res) => { const iv = setInterval(() => { if (document.querySelector(".cm-content")?.textContent?.includes("Three roots")) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); return +(performance.now() - t0).toFixed(0); });
  const times = [];
  for (let i = 0; i < 5; i++) {
    const t = await page.evaluate(async (k) => { const links = [...document.querySelectorAll('nav[aria-label="Notes"] a.row')]; const long = links.find((a) => a.textContent.includes("The long note")); const other = links.find((a) => !a.textContent.includes("The long note")); (k % 2 ? other : long).click(); const want = k % 2 ? "" : "Section 3"; const t0 = performance.now(); if (!want) { await new Promise((r) => setTimeout(r, 200)); return null; } await new Promise((res) => { const iv = setInterval(() => { if (document.querySelector(".cm-content")?.textContent?.includes(want)) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); return performance.now() - t0; }, i * 2);
    if (t) times.push(t);
    await page.waitForTimeout(300);
  }
  out.openLongWarm = { medianMs: +pct(times, 0.5).toFixed(0), worstMs: +Math.max(...times).toFixed(0), runs: times.length };
  await ctx.close();
}
{ // cold: a fresh page straight onto the long note
  const ctx = await b.newContext({ ...DEVICES.desktop }); await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page); if (RATE > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: RATE });
  const t0 = Date.now();
  await page.goto(BASE + "/notes/n-long");
  await page.waitForSelector(".cm-h2");
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  out.openLongCold = { ms: Date.now() - t0 };
  await ctx.close();
}
// 2. Typing in the long note (near its end and in the middle) with the probe.
for (const where of ["end", "middle"]) {
  const { ctx, page } = await fresh("/notes/n-long?test=1");
  await page.waitForSelector(".cm-h2"); await page.waitForTimeout(1000);
  await page.locator(".cm-content").click();
  await page.keyboard.press(where === "end" ? "Control+End" : "Control+Home");
  if (where === "middle") for (let i = 0; i < 40; i++) await page.keyboard.press("PageDown");
  await page.waitForTimeout(500);
  await page.evaluate(() => { window.__keys = []; });
  await page.keyboard.type("the quick brown fox jumps over the lazy dog, then naps. ".repeat(1), { delay: 60 });
  await page.waitForTimeout(400);
  const keys = await page.evaluate(() => window.__keys ?? []);
  out[`type_${where}`] = { keys: keys.length, medianMs: +pct(keys, 0.5).toFixed(1), p95Ms: +pct(keys, 0.95).toFixed(1), worstMs: +Math.max(...keys).toFixed(1) };
  await ctx.close();
}
// 3. Scrolling the long note.
{
  const { ctx, page } = await fresh("/notes/n-long");
  await page.waitForSelector(".cm-h2"); await page.waitForTimeout(1000);
  const box = await page.locator(".note .scroll").boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.evaluate(() => (window.__frames = []));
  for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, 800); await page.waitForTimeout(16); }
  for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, -800); await page.waitForTimeout(16); }
  const f = await page.evaluate(() => window.__frames);
  out.scroll = { frames: f.length, medianMs: +pct(f, 0.5).toFixed(1), p95Ms: +pct(f, 0.95).toFixed(1), worstMs: +Math.max(...f).toFixed(1), over50: f.filter((x) => x > 50).length };
  await ctx.close();
}
await b.close(); stop();
console.log(JSON.stringify(out, null, 1));
