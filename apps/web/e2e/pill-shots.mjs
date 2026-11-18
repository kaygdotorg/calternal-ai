// Frames of message text scrolled under the floating pills (title, window controls, composer), card and canvas, to /tmp/pill-shots (attach to the issue, never commit). Usage: node e2e/with-preview.mjs e2e/pill-shots.mjs [dir]
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/pill-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
for (const scheme of ["light", "dark"]) for (const mode of ["card", "canvas"]) {
  const ctx = await browser.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
  const p = await ctx.newPage();
  await p.goto(BASE + "/threads/t-idle" + (mode === "canvas" ? "?canvas=1" : "")); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500);
  await p.evaluate(() => { for (const e of document.querySelectorAll("*")) if (e.scrollHeight > e.clientHeight + 200 && /auto|scroll/.test(getComputedStyle(e).overflowY)) e.scrollTop = 400; });
  await p.waitForTimeout(600);
  await p.screenshot({ path: `${dir}/${mode}-${scheme}.png`, clip: { x: 330, y: 0, width: 1170, height: 970 } });
  await ctx.close();
}
await browser.close(); await stop();
