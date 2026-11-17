// Long threads in server mode: open and scroll a 10,000 message thread and a thread with one 5 MB agent message through the real adapter (the web app against `e2e/server-double.mjs`), and watch a turn
// stream into the bottom of each. Reports frames (a frame is 16.7 ms), long tasks (50 ms or more), time to the first visible message, and JS heap, at 1x and 4x CPU slowdown.
// Usage: CAL_SERVER=http://127.0.0.1:34200 node e2e/long-server.mjs   (a `vite preview` of the build serves the app and proxies /api; start the double first)
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const T10K = "019a8c00-0000-7000-8000-000000000001", T5MB = "019a8c00-0000-7000-8000-000000000002";
const pct = (xs, p) => { const s = [...xs].sort((a, c) => a - c); return s.length ? s[Math.min(s.length - 1, Math.floor(p * (s.length - 1)))] : 0; };
const SAMPLER = `window.__frames = []; window.__long = []; (() => { let last = performance.now(); const f = (t) => { window.__frames.push(t - last); last = t; requestAnimationFrame(f); }; requestAnimationFrame(f); new PerformanceObserver((l) => { for (const e of l.getEntries()) window.__long.push(e.duration); }).observe({ type: "longtask", buffered: true }); })();`;
const stop = await serve(); const b = await chromium.launch({ args: ["--enable-precise-memory-info"] });
const stats = async (page) => { const f = await page.evaluate(() => window.__frames.splice(0)); const l = await page.evaluate(() => window.__long.splice(0)); return { frames: f.length, median: +pct(f, 0.5).toFixed(1), p95: +pct(f, 0.95).toFixed(1), worst: f.length ? +Math.max(...f).toFixed(1) : 0, over20: f.filter((x) => x > 20).length, longTasks: l.length, longest: l.length ? +Math.max(...l).toFixed(0) : 0 }; };
const heap = async (cdp) => { const m = await cdp.send("Performance.getMetrics"); return +(m.metrics.find((x) => x.name === "JSHeapUsedSize").value / 1048576).toFixed(0); };
for (const slow of [1, 4]) {
  console.log(`\n=== CPU slowdown ${slow}x`);
  for (const [name, id, last] of [["10,000 messages", T10K, true], ["one 5 MB message", T5MB, true]]) {
    const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
    await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", "x"); localStorage.setItem("cal.server", "1"); } catch {} });
    await ctx.addInitScript(SAMPLER);
    const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page); await cdp.send("Performance.enable");
    if (slow > 1) await cdp.send("Emulation.setCPUThrottlingRate", { rate: slow });
    await page.goto(BASE + "/threads?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
    const h0 = await heap(cdp); await page.evaluate(() => { window.__frames.length = 0; window.__long.length = 0; });
    const t0 = Date.now();
    await page.evaluate((id) => { history.pushState(null, "", `/threads/${id}`); dispatchEvent(new PopStateEvent("popstate")); }, id);
    await page.waitForSelector(".msg", { timeout: 120000 }); const tFirst = Date.now() - t0;
    await page.waitForTimeout(1500);
    const open = await stats(page); const h1 = await heap(cdp);
    // scroll: 100 wheel steps up, then 100 down
    const box = await page.locator(".list").first().boundingBox(); await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, -900); await page.waitForTimeout(8); }
    for (let i = 0; i < 100; i++) { await page.mouse.wheel(0, 700); await page.waitForTimeout(8); }
    const scroll = await stats(page); const h2 = await heap(cdp);
    // a turn streams into the bottom (20 s of words arriving every 100 ms, a checkpoint every 500 ms)
    await page.keyboard.press("End").catch(() => {});
    await page.locator("#composer-field").click(); await page.locator("#composer-field").fill("go"); await page.keyboard.press("Enter");
    await page.waitForTimeout(1500); await page.evaluate(() => { window.__frames.length = 0; window.__long.length = 0; });
    await page.waitForTimeout(10000);
    const stream = await stats(page); const h3 = await heap(cdp);
    console.log(`${name}: first message after ${tFirst} ms | open: ${JSON.stringify(open)} | scroll: ${JSON.stringify(scroll)} | streaming 10 s: ${JSON.stringify(stream)} | JS heap MiB: before ${h0}, open ${h1}, after scroll ${h2}, after streaming ${h3} | DOM elements ${await page.evaluate(() => document.querySelectorAll("*").length)}`);
    await ctx.close();
    await new Promise((r) => setTimeout(r, 12000)); // let the double's turn finish
  }
}
await b.close(); stop();
