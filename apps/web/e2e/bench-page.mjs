// Runs the in-page benchmark (src/bench.ts) in Chromium to check the runner itself and give a baseline at 1x: node e2e/bench-page.mjs
import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const ctx = await b.newContext({ ...DEVICES.desktop }); const page = await ctx.newPage();
await page.goto(BASE + "/notes/n-1?bench=1&firstrun=0&test=1");
await page.waitForFunction(() => window.__bench?.status === "done" || String(window.__bench?.status).startsWith("error"), null, { timeout: 120000 });
console.log(JSON.stringify(await page.evaluate(() => window.__bench), null, 1));
await b.close(); stop();
