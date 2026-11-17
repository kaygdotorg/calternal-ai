import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const { page } = await open(b, "desktop", "dark", "/threads/t-idle");
await page.locator("#side-field").click(); await page.keyboard.press("ArrowLeft"); await page.waitForTimeout(200);
console.log(JSON.stringify(await page.evaluate(() => { const f = document.activeElement; const inn = f.closest(".in"); const c = inn && getComputedStyle(inn); return { act: f.id, input: document.documentElement.dataset.input, fv: f.matches(":focus-visible"), inHas: inn?.matches(":has(:focus-visible)"), out: c?.outlineStyle, w: c?.outlineWidth, cls: inn?.className }; })));
await b.close(); stop();
