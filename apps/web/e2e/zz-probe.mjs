import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, "desktop", "dark", "/threads?canvas=1");
await p.waitForTimeout(2200);
for (let i = 0; i < 7; i++) { await p.keyboard.press("Alt+Shift+KeyN"); await p.waitForTimeout(200); }
const f = await p.locator("[data-win][data-focused]").boundingBox();
await p.mouse.move(f.x + 100, f.y + f.height / 2);
console.log(JSON.stringify(await p.evaluate(() => JSON.parse(localStorage.getItem("cal.canvas")).cols.flatMap((c) => c.wins.map((w) => w.kind + ":" + w.item)))));
console.log(JSON.stringify(await p.evaluate(() => { const w = document.querySelector("[data-win][data-focused]"); const fd = w.querySelector("[role=feed]"); return { feed: fd ? [fd.scrollHeight, fd.clientHeight, fd.scrollTop] : null }; })));
await p.mouse.wheel(0, -9999); await p.waitForTimeout(800);
console.log(JSON.stringify(await p.evaluate(() => ({ mounted: document.querySelectorAll("[data-win]").length, x: JSON.parse(localStorage.getItem("cal.canvas")).x }))));
await browser.close(); await stop();
