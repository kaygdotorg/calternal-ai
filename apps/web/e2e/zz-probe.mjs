import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
const { page: p } = await open(browser, "desktop", "dark", "/threads?canvas=1");
await p.waitForTimeout(2200);
const first = await p.locator("[data-win]").first().boundingBox();
await p.mouse.move(first.x + 100, first.y + first.height / 2);
for (let i = 0; i < 7; i++) { await p.keyboard.press("Alt+Shift+KeyN"); await p.waitForTimeout(200); }
const x = () => p.evaluate(() => JSON.parse(localStorage.getItem("cal.canvas")).x);
console.log("x before", await x(), JSON.stringify(await p.evaluate(({ px, py }) => { const e = document.elementFromPoint(px, py); const w = e?.closest("[data-win]"); return { kind: w?.dataset.kind, item: w?.dataset.item, focused: w?.hasAttribute("data-focused"), tag: e?.tagName + "." + e?.className }; }, { px: first.x + 100, py: first.y + first.height / 2 })));
for (let i = 0; i < 6; i++) { await p.mouse.wheel(0, -9999); await p.waitForTimeout(700); console.log("x", await x()); }
await browser.close(); await stop();
