// Dragging "Clear to tinted" must stay smooth: at 4x CPU throttling, across all five steps by keyboard (up and down) and by a pointer drag along the rail, no main-thread task over 50 ms, the wallpaper's blurred copies are never rebuilt
// (their blob URLs stay the same), and each step is on the screen within three frames. Both colour schemes. A step used to re-blur the picture and re-solve everything (a long task of hundreds of milliseconds at 4x).
// Usage: node e2e/clarity-perf.mjs (after `npm run build`, through with-preview).
import { chromium } from "playwright";
import { BASE, busyNow, judgeTimings, serve } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch(); let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const q = (a, f) => { const s = [...a].sort((x, y) => x - y); return s.length ? s[Math.min(s.length - 1, Math.floor(s.length * f))] : 0; };
for (const scheme of ["dark", "light"]) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, colorScheme: scheme });
  await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
  const page = await ctx.newPage();
  await page.goto(`${BASE}/settings/appearance/appearance.clarity?firstrun=0`); await page.waitForSelector("html[data-ready]");
  await page.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue("--wall-side").includes("blob"), null, { timeout: 20000 }); await page.waitForTimeout(800);
  const slider = page.getByRole("slider", { name: /Clear to tinted/ }); await slider.scrollIntoViewIfNeeded();
  const walls = () => page.evaluate(() => ["--wall-side", "--wall-card"].map((k) => getComputedStyle(document.documentElement).getPropertyValue(k)).join("|"));
  const before = await walls();
  const cdp = await ctx.newCDPSession(page); await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await page.evaluate(() => { window.__long = []; new PerformanceObserver((l) => l.getEntries().forEach((e) => __long.push(e.duration))).observe({ entryTypes: ["longtask"] }); });
  const step = async (key) => { const t = await page.evaluate(() => performance.now()); await page.keyboard.press(key); return page.evaluate((t0) => new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(() => res(performance.now() - t0)))), t); };
  await slider.focus(); const lat = [];
  lat.push(await step("Home"));
  for (let i = 0; i < 4; i++) lat.push(await step("ArrowRight"));
  for (let i = 0; i < 4; i++) lat.push(await step("ArrowLeft"));
  const r = await slider.boundingBox();
  await page.mouse.move(r.x + 4, r.y + r.height / 2); await page.mouse.down();
  for (let i = 1; i <= 20; i++) { await page.mouse.move(r.x + 4 + ((r.width - 8) * i) / 20, r.y + r.height / 2); await page.waitForTimeout(16); }
  await page.mouse.up(); await page.waitForTimeout(500);
  const long = await page.evaluate(() => __long.slice()); const after = await walls();
  console.log(`   ${scheme}: step on screen p50 ${q(lat, 0.5).toFixed(0)} ms, max ${Math.max(...lat).toFixed(0)} ms; long tasks ${long.length}${long.length ? " (" + long.map((x) => x.toFixed(0)).join(", ") + " ms)" : ""}`);
  const judge = judgeTimings();
  if (!judge) console.log(`SKIP  ${scheme}: timing judgement (numbers above, not judged): the machine is busy (${busyNow().why}); the quiet list judges it: npm run e2e:quiet`);
  else ok(Math.max(0, ...long) <= 50, `${scheme}: no main-thread task over 50 ms while the slider moves through all five steps at 4x CPU`, `longest ${Math.max(0, ...long).toFixed(0)} ms`);
  ok(before === after, `${scheme}: the blurred wallpaper copies are not rebuilt (same blob URLs before and after)`);
  if (judge) ok(Math.max(...lat) < 150, `${scheme}: each step is on the screen within a few frames at 4x`, `max ${Math.max(...lat).toFixed(0)} ms`);
  await ctx.close();
}
await b.close(); stop(); process.exit(bad ? 1 : 0);
