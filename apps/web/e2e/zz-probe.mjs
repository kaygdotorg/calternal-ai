import { launch, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const ctx = await browser.newContext({ viewport: { width: 844, height: 250 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, colorScheme: "dark" });
await ctx.addInitScript(() => localStorage.setItem("cal.firstrun", "1"));
const p = await ctx.newPage(); await p.goto(process.env.CAL_URL + "/threads/t-secret"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1200);
await p.screenshot({ path: "/tmp/ls-250.png" });
console.log(JSON.stringify(await p.evaluate(() => { const b = document.querySelector("[aria-label='Waiting on you']"); const a = b.querySelector(".act"); const h = b.querySelector("h2"); return { bar: b.getBoundingClientRect().toJSON(), act: a?.getBoundingClientRect().toJSON(), h2: h?.getBoundingClientRect().toJSON(), cls: a?.className, mq: matchMedia("(max-height: 520px)").matches }; })));
await browser.close(); await stop();
