import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const ctx = await b.newContext({ ...DEVICES.desktop }); const page = await ctx.newPage();
await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
console.log(await page.evaluate(() => { const e = document.querySelector("aside.side .scroll"); const c = getComputedStyle(e); return JSON.stringify({ cls: e.className, mask: c.maskImage.slice(0, 40), wk: c.webkitMaskImage?.slice(0, 40) }); }));
await page.locator(".msg").nth(2).hover(); await page.waitForTimeout(300);
console.log(await page.evaluate(() => document.querySelector(".msg .acts")?.outerHTML.slice(0, 200)));
await b.close(); stop();
