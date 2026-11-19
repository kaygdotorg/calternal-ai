import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, "desktop", "dark", "/threads?scenario=empty-with-prompt");
await p.waitForTimeout(1200);
console.log(JSON.stringify(await p.evaluate(() => ({ side: document.querySelector("aside.side")?.innerText.slice(0, 300), url: location.href }))));
await browser.close(); await stop();
