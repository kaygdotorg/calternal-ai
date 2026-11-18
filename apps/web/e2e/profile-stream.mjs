// Where the main thread's time goes while a turn streams into a long thread (the 5 MB or the 10,000-message one of e2e/server-double.mjs) at 4x CPU slowdown: a CPU profile of the streaming window,
// top functions by self time, and the frames over 20 ms. Usage: CAL_SERVER=http://127.0.0.1:34210 CAL_URL=http://127.0.0.1:4395 node e2e/profile-stream.mjs [5mb|10k]
import { chromium } from "playwright";
import { BASE } from "./lib.mjs";
const T = { "10k": "019a8c00-0000-7000-8000-000000000001", "5mb": "019a8c00-0000-7000-8000-000000000002" }[process.argv[2] ?? "5mb"];
const b = await chromium.launch(); const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", "x"); localStorage.setItem("cal.server", "1"); } catch {} });
await ctx.addInitScript(`window.__frames = []; (() => { let last = performance.now(); const f = (t) => { window.__frames.push(t - last); last = t; requestAnimationFrame(f); }; requestAnimationFrame(f); })();`);
const page = await ctx.newPage(); const cdp = await ctx.newCDPSession(page);
await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
await page.goto(BASE + "/threads?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
await page.evaluate((id) => { history.pushState(null, "", `/threads/${id}`); dispatchEvent(new PopStateEvent("popstate")); }, T);
await page.waitForSelector(".msg", { timeout: 120000 }); await page.waitForTimeout(1500);
await page.keyboard.press("End").catch(() => {});
await page.locator("#composer-field").click(); await page.locator("#composer-field").fill("go"); await page.keyboard.press("Enter");
await page.waitForTimeout(1500); await page.evaluate(() => { window.__frames.length = 0; });
await cdp.send("Profiler.enable"); await cdp.send("Profiler.setSamplingInterval", { interval: 200 }); await cdp.send("Profiler.start");
await page.waitForTimeout(10000);
const { profile } = await cdp.send("Profiler.stop");
const frames = await page.evaluate(() => window.__frames.splice(0));
const self = new Map(); const byId = new Map(profile.nodes.map((n) => [n.id, n]));
const dt = profile.timeDeltas; profile.samples.forEach((id, i) => { const n = byId.get(id); const k = `${n.callFrame.functionName || "(anon)"} ${n.callFrame.url.split("/").slice(-2).join("/")}:${n.callFrame.lineNumber + 1}`; self.set(k, (self.get(k) ?? 0) + (dt[i] ?? 0)); });
const total = [...self.values()].reduce((a, c) => a + c, 0);
console.log("late frames (index of 600, ms):", frames.map((f, i) => [i, +f.toFixed(0)]).filter((x) => x[1] > 20).map((x) => x.join(":")).join(" "));
console.log(`frames ${frames.length}, over 20 ms: ${frames.filter((f) => f > 20).length}, worst ${Math.max(...frames).toFixed(0)} ms | profile total ${(total / 1000).toFixed(0)} ms in 10 s at 4x`);
for (const [k, v] of [...self].sort((a, c) => c[1] - a[1]).slice(0, 22)) console.log(`${(v / 1000).toFixed(0).padStart(6)} ms  ${k}`);
await b.close();
