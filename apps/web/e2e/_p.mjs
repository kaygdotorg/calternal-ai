import { chromium } from "playwright";
const b = await chromium.launch();
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 }); const page = await ctx.newPage();
page.on("pageerror", e => console.log("E:", String(e).slice(0,200)));
const t0 = Date.now();
await page.goto("http://127.0.0.1/ai/threads/t-idle"); await page.waitForSelector("html[data-ready]"); console.log("ready", Date.now() - t0);
for (const ms of [0, 500, 1500, 4000]) { await page.waitForTimeout(ms); console.log(ms, JSON.stringify(await page.evaluate(() => ({ wall: getComputedStyle(document.documentElement).getPropertyValue("--wall-side").slice(0, 50), ap: localStorage.getItem("cal.appearance")?.slice(0, 60), sheet: !!document.querySelector('[role=dialog]') })))); }
await page.screenshot({ path: "/tmp/prev-fresh.png" });
await b.close();
