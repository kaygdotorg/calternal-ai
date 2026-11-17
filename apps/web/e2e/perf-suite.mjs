// Measured costs of the five things that matter (docs/ui-performance.md): the first load, scrolling the 10,000-message thread, switching threads, opening a
// sheet over the wallpaper, and opening the palette and typing in it. Each reports frame times from requestAnimationFrame (a frame is 16.7 ms), long tasks (50 ms or
// more) and, where it applies, the time to the visible result. Run at 1x and 4x CPU slowdown. Usage: node e2e/perf-suite.mjs [--slow] (after `npm run build`).
import { chromium } from "playwright";
import { serve, open, DEVICES, BASE } from "./lib.mjs";

const SLOW = process.argv.includes("--slow") ? 4 : 1;
const stop = await serve();
const b = await chromium.launch();
const out = {};
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))]; };
const SAMPLER = `window.__frames = []; window.__long = []; (() => { let last = performance.now(); const f = (t) => { window.__frames.push(t - last); last = t; requestAnimationFrame(f); }; requestAnimationFrame(f); new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__long.push(e.duration); }).observe({ type: "longtask", buffered: true }); })();`;
const frames = async (page) => { const f = await page.evaluate(() => window.__frames.splice(0)); const l = await page.evaluate(() => window.__long.splice(0)); return { frames: f.length, median: +pct(f, 0.5).toFixed(1), p95: +pct(f, 0.95).toFixed(1), worst: +Math.max(...f).toFixed(1), over20: f.filter((x) => x > 20).length, longTasks: l.length, longestTask: l.length ? +Math.max(...l).toFixed(0) : 0 }; };
const context = async (path) => {
  const { ctx, page } = await open(b, "desktop", "light", path);
  const cdp = await ctx.newCDPSession(page);
  if (SLOW > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: SLOW });
  return { ctx, page, cdp };
};

// 1. First load: a fresh context, no cache.
{
  const ctx = await b.newContext({ ...DEVICES.desktop });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch {} });
  const page = await ctx.newPage();
  const cdp = await ctx.newCDPSession(page);
  if (SLOW > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: SLOW });
  await page.addInitScript(`window.__paints = []; new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__paints.push([e.name, e.startTime]); }).observe({ type: "paint", buffered: true }); new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__lcp = e.startTime; }).observe({ type: "largest-contentful-paint", buffered: true });`);
  await page.goto(BASE + "/threads/t-run", { waitUntil: "load" });
  await page.waitForSelector("html[data-ready]");
  await page.waitForSelector("h1");
  await page.waitForTimeout(1500);
  const r = await page.evaluate(() => {
    const res = performance.getEntriesByType("resource");
    const nav = performance.getEntriesByType("navigation")[0];
    const by = (re) => res.filter((x) => re.test(x.name)).reduce((s, x) => s + (x.encodedBodySize || x.transferSize || 0), 0);
    return { fcp: Math.round(window.__paints.find((p) => p[0] === "first-contentful-paint")?.[1] ?? 0), lcp: Math.round(window.__lcp ?? 0), dcl: Math.round(nav.domContentLoadedEventEnd), requests: res.length + 1, kib: Math.round((res.reduce((s, x) => s + (x.encodedBodySize || 0), nav.encodedBodySize)) / 1024), js: Math.round(by(/\.js/) / 1024), css: Math.round(by(/\.css/) / 1024), fonts: Math.round(by(/woff2/) / 1024), images: Math.round(by(/\.(jpg|png|webp|blob)|blob:/) / 1024) };
  });
  out.firstLoad = r;
  await ctx.close();
}
// 2. Scrolling the 10,000-message thread: 100 wheel steps up, 100 down.
{
  const { ctx, page } = await context("/threads/t-long");
  await page.addInitScript(SAMPLER); await page.reload(); await page.waitForSelector("h1"); await page.waitForTimeout(800);
  await frames(page);
  const feed = page.getByRole("feed");
  const box = await feed.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, -700); await page.waitForTimeout(16); }
  for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, 700); await page.waitForTimeout(16); }
  out.scroll10k = await frames(page);
  await ctx.close();
}
// 3. Switching threads: click a row, time to the new title painted.
{
  const { ctx, page } = await context("/threads/t-run");
  await page.addInitScript(SAMPLER); await page.reload(); await page.waitForSelector("h1"); await page.waitForTimeout(800);
  const times = [];
  const rows = page.locator("a.row");
  const n = await rows.count();
  await frames(page);
  for (let i = 0; i < 12; i++) {
    const t = await page.evaluate(async (k) => { const links = [...document.querySelectorAll("a.row")]; const a = links[k % links.length]; const want = a.querySelector(".t")?.textContent; const t0 = performance.now(); a.click(); await new Promise((res) => { const iv = setInterval(() => { if (document.querySelector("h1")?.textContent === want) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); return performance.now() - t0; }, i + 1);
    times.push(t); await page.waitForTimeout(150);
  }
  out.switchThread = { ...(await frames(page)), clickToPaintMedian: +pct(times, 0.5).toFixed(0), clickToPaintP95: +pct(times, 0.95).toFixed(0), rows: n };
  await ctx.close();
}
// 4. Opening a sheet over the wallpaper (Appearance), 6 times.
{
  const { ctx, page } = await context("/threads/t-run");
  await page.addInitScript(SAMPLER); await page.reload(); await page.waitForSelector("h1"); await page.waitForTimeout(800);
  await frames(page);
  const times = [];
  for (let i = 0; i < 6; i++) {
    const t = await page.evaluate(async () => { const t0 = performance.now(); document.querySelector('button[aria-label="Search"]').click(); await new Promise((res) => { const iv = setInterval(() => { const d = document.querySelector('[role=dialog][data-ready]'); if (d) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); const ms = performance.now() - t0; document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true })); return ms; }, null);
    times.push(t); await page.keyboard.press("Escape"); await page.waitForTimeout(500);
  }
  out.openSheet = { ...(await frames(page)), openToReadyMedian: +pct(times, 0.5).toFixed(0), openToReadyP95: +pct(times, 0.95).toFixed(0) };
  await ctx.close();
}
// 5. The palette: open, then type a 30 character line.
{
  const { ctx, page } = await context("/threads/t-run");
  await page.addInitScript(SAMPLER); await page.reload(); await page.waitForSelector("h1"); await page.waitForTimeout(800);
  await frames(page);
  const t = await page.evaluate(async () => { const t0 = performance.now(); document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, code: "KeyK", bubbles: true })); window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true, code: "KeyK", bubbles: true })); await new Promise((res) => { const iv = setInterval(() => { if (document.querySelector(".pal-input")) { clearInterval(iv); res(); } }, 1); }); await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))); return performance.now() - t0; });
  await page.waitForTimeout(600);
  const keys = await page.evaluate(async () => { const el = document.querySelector(".pal-input"); const out = []; const line = "call plumber friday 5pm about journal"; for (let i = 1; i <= line.length; i++) { const t0 = performance.now(); el.value = line.slice(0, i); el.dispatchEvent(new Event("input", { bubbles: true })); await new Promise((r) => requestAnimationFrame(() => r())); out.push(performance.now() - t0); } return out; });
  out.palette = { ...(await frames(page)), openMs: +t.toFixed(0), keystrokeMedian: +pct(keys, 0.5).toFixed(1), keystrokeP95: +pct(keys, 0.95).toFixed(1) };
  await ctx.close();
}
await b.close(); stop();
console.log(JSON.stringify({ cpuSlowdown: SLOW, ...out }, null, 1));
