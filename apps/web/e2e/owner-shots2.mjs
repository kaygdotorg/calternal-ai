// Frames for the second owner round (#33): the Selector tabs, a lone window without a full screen control against windows with one, the window menu without Width and Look, and a menu on a short screen with its scroll cue. Usage: node e2e/owner-shots2.mjs [outdir]
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/owner-shots2"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
for (const scheme of ["dark", "light"]) {
  const ctx = await browser.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme });
  await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
  const p = await ctx.newPage();
  await p.goto(BASE + "/threads?canvas=1"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2800);
  await p.screenshot({ path: `${dir}/${scheme}-1-tabs-inbox.png`, clip: { x: 0, y: 150, width: 340, height: 330 } });
  await p.locator("aside.side [role=tab]", { hasText: "All Threads" }).click(); await p.waitForTimeout(110);
  await p.screenshot({ path: `${dir}/${scheme}-2-tabs-mid-glide.png`, clip: { x: 0, y: 150, width: 340, height: 330 } });
  await p.waitForTimeout(600);
  await p.screenshot({ path: `${dir}/${scheme}-3-tabs-all.png`, clip: { x: 0, y: 150, width: 340, height: 330 } });
  await p.screenshot({ path: `${dir}/${scheme}-4-windows-with-full-screen.png`, clip: { x: 330, y: 0, width: 1170, height: 90 } });
  while ((await p.locator("[data-win]").count()) > 1) { await p.keyboard.press("Alt+Shift+KeyW"); await p.waitForTimeout(150); }
  await p.waitForTimeout(500);
  await p.screenshot({ path: `${dir}/${scheme}-5-lone-window-no-full-screen.png`, clip: { x: 330, y: 0, width: 1170, height: 90 } });
  await p.keyboard.press("Alt+Shift+KeyN"); await p.waitForTimeout(400);
  await p.locator("[data-win]").first().getByRole("button", { name: "More" }).click(); await p.waitForTimeout(700);
  await p.screenshot({ path: `${dir}/${scheme}-6-window-menu.png` });
  await p.keyboard.press("Escape"); await p.setViewportSize({ width: 1500, height: 330 }); await p.waitForTimeout(500);
  await p.locator("[data-win]").first().getByRole("button", { name: "More" }).click(); await p.waitForTimeout(800);
  await p.screenshot({ path: `${dir}/${scheme}-7-menu-short-screen.png`, clip: { x: 700, y: 0, width: 800, height: 330 } });
  await ctx.close();
}
await browser.close(); await stop();
console.log("done", dir);
