import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
for (const [n, dev] of [["desktop", DEVICES.desktop], ["phone", DEVICES.phone]]) {
  const ctx = await b.newContext({ ...dev }); const page = await ctx.newPage();
  await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(1500);
  await page.screenshot({ path: `/tmp/fresh-${n}.png` });
  console.log(n, JSON.stringify(await page.evaluate(() => ({ wall: getComputedStyle(document.documentElement).getPropertyValue("--wall-side").slice(0, 60), ls: Object.keys(localStorage), buttons: [...document.querySelectorAll("button")].map((x) => x.getAttribute("aria-label") || x.textContent.trim()).filter(Boolean).slice(0, 30) }))));
  await ctx.close();
}
await b.close(); stop();
