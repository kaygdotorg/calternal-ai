// Frames for the owner-feedback round (#33): the Inbox rows, a canvas window's top right with the side panel closed, the resize handle between windows on hover, and canvas=0 with the side panel closed. Usage: node e2e/owner-shots.mjs [outdir]
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/owner-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
for (const scheme of ["dark", "light"]) {
  const ctx = await browser.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
  const p = await ctx.newPage();
  await p.goto(BASE + "/threads?canvas=1"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2800);
  await p.keyboard.press("Control+Shift+Backslash"); await p.waitForTimeout(600); // the side panel closed: no button of its own
  await p.screenshot({ path: `${dir}/${scheme}-1-canvas-panel-closed.png` });
  const wins = p.locator("[data-win]"); const b = await wins.nth(0).boundingBox();
  await p.mouse.move(b.x + b.width + 4, b.y + b.height / 2); await p.waitForTimeout(500);
  await p.screenshot({ path: `${dir}/${scheme}-2-resize-handle.png`, clip: { x: b.x + b.width - 120, y: b.y + b.height / 2 - 160, width: 240, height: 320 } });
  await p.goto(BASE + "/threads/t-idle?canvas=0"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(1500);
  await p.screenshot({ path: `${dir}/${scheme}-3-canvas0-panel-closed.png` });
  await p.getByRole("button", { name: "Thread actions" }).click(); await p.waitForTimeout(400);
  await p.screenshot({ path: `${dir}/${scheme}-4-canvas0-menu.png` });
  await ctx.close();
}
await browser.close(); await stop();
console.log("done", dir);
