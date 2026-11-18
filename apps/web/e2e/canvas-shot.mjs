import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const [, , device = "desktop", scheme = "dark", out = "/tmp/canvas"] = process.argv;
const { page, ctx } = await open(browser, device, scheme, "/threads/t-idle?canvas=1");
await page.waitForTimeout(2500);
await page.screenshot({ path: `${out}-${device}-${scheme}.png` });
console.log(await page.evaluate(() => JSON.stringify({ wins: [...document.querySelectorAll("[data-win]")].map((w) => { const r = w.getBoundingClientRect(); return [w.dataset.kind, Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)]; }), url: location.pathname })));
await browser.close(); await stop();
