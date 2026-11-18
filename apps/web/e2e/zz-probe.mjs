import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, "phone", "light", "/threads?scenario=empty");
await p.waitForTimeout(1200);
console.log(JSON.stringify(await p.evaluate(() => ({ wins: document.querySelectorAll("[data-win]").length, none: document.querySelector(".none")?.textContent, text: [...document.querySelectorAll("main *")].filter((e) => e.children.length === 0 && e.textContent.trim()).map((e) => e.textContent.trim()).slice(0, 12) }))));
await browser.close(); await stop();
