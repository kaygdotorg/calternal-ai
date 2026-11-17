// Screenshots from a headed, GPU-backed Chrome (backdrop-filter is drawn), the browser the owner uses: for the faults seen there (a band under the card's top edge with content scrolled under it,
// an edge and band at the bottom of the right sidebar). Run on a machine with Chrome started with --remote-debugging-port=9222. Usage: BASE=http://host/ai node gpu-shots.mjs <outDir> [scheme]
import { chromium } from "playwright-core";
import { mkdirSync } from "node:fs";
const BASE = process.env.BASE, OUT = process.argv[2], SCHEME = process.argv[3] ?? "dark";
mkdirSync(OUT, { recursive: true });
const b = await chromium.connectOverCDP("http://127.0.0.1:9222");
const ctx = b.contexts()[0] ?? await b.newContext();
const page = await ctx.newPage();
await page.setViewportSize({ width: 1500, height: 970 });
await page.emulateMedia({ colorScheme: SCHEME });
await page.goto(BASE + "/threads/t-idle?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2500);
await page.screenshot({ path: `${OUT}/full.png` });
await page.screenshot({ path: `${OUT}/bottom-right.png`, clip: { x: 1100, y: 870, width: 400, height: 100 } });
await page.evaluate(() => { const s = [...document.querySelectorAll(".list *")].find((e) => e.scrollHeight > e.clientHeight + 50); s.scrollTop = 700; });
await page.waitForTimeout(600);
await page.screenshot({ path: `${OUT}/top-scrolled.png`, clip: { x: 240, y: 0, width: 920, height: 170 } });
console.log("ok", await page.evaluate(() => navigator.userAgent.slice(-40)));
await page.close(); await b.close();
