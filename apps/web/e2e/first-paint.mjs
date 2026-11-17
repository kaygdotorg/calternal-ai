// First paint of a fresh visit, measured: first-contentful-paint and the moment the app is ready (data-ready), median and p90 of N cold loads (new context each, so no cache), over the built app.
// Usage: node e2e/first-paint.mjs [N]   (after `npm run build`). Also prints the fonts preloaded before the first paint.
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const N = Number(process.argv[2] ?? 15);
const stop = await serve(); const b = await chromium.launch();
const fcp = [], ready = [], pre = [];
for (let i = 0; i < N; i++) {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, deviceScaleFactor: 2 });
  const page = await ctx.newPage();
  await page.goto(BASE + "/threads/t-idle?firstrun=0", { waitUntil: "load" });
  await page.waitForSelector("html[data-ready]");
  const m = await page.evaluate(() => ({ fcp: performance.getEntriesByName("first-contentful-paint")[0]?.startTime ?? 0, ready: performance.now(), pre: [...document.querySelectorAll('link[rel=preload][as=font]')].length, scheme: document.documentElement.dataset.scheme }));
  fcp.push(m.fcp); ready.push(m.ready); pre.push(m.pre);
  await ctx.close();
}
const q = (a, p) => [...a].sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(a.length * p))].toFixed(0);
console.log(`fcp   median ${q(fcp, 0.5)} ms  p90 ${q(fcp, 0.9)} ms\nready median ${q(ready, 0.5)} ms  p90 ${q(ready, 0.9)} ms\nfont preloads in head: ${pre[0]}`);
await b.close(); stop();
