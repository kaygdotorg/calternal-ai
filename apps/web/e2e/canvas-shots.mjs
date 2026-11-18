// Screenshots of the canvas for the review against the rules (docs/ui-shell-status.md): 1500x970 at 2x in both schemes (three windows; one window; a transparent window; the window menu; full screen; a tooltip) and a phone. Usage: CAL_URL=... node e2e/canvas-shots.mjs <dir>
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/canvas-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
const ctxOf = async (scheme, phone) => { const ctx = await browser.newContext(phone ? { viewport: { width: 390, height: 844 }, deviceScaleFactor: 2, isMobile: true, hasTouch: true, colorScheme: scheme } : { viewport: { width: 1500, height: 970 }, deviceScaleFactor: 2, colorScheme: scheme, reducedMotion: "no-preference" }); await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } }); return ctx; };
const goto = async (ctx, q = "/threads?canvas=1") => { const p = await ctx.newPage(); await p.goto(BASE + q); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2800); return p; };
for (const scheme of ["dark", "light"]) {
  const ctx = await ctxOf(scheme);
  const p = await goto(ctx);
  await p.screenshot({ path: `${dir}/${scheme}-1-three.png` });
  await p.locator("[data-win]").first().getByRole("button", { name: "More" }).click(); await p.waitForTimeout(500);
  await p.screenshot({ path: `${dir}/${scheme}-2-menu.png` });
  await p.keyboard.press("Escape");
  await p.locator("[data-win]").first().getByRole("button", { name: "Full screen" }).hover(); await p.waitForTimeout(1100);
  await p.screenshot({ path: `${dir}/${scheme}-3-tooltip.png` });
  await p.mouse.move(5, 5);
  await p.evaluate(() => localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "clear", width: null }))); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2200);
  await p.screenshot({ path: `${dir}/${scheme}-4-transparent.png` });
  await p.evaluate(() => localStorage.setItem("cal.canvas.prefs", JSON.stringify({ look: "card", width: null }))); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2200);
  await p.locator("[data-win]").first().getByRole("button", { name: "Full screen" }).click(); await p.waitForTimeout(800);
  await p.screenshot({ path: `${dir}/${scheme}-5-fullscreen.png` });
  await p.keyboard.press("Alt+Shift+KeyF"); await p.waitForTimeout(300);
  while ((await p.locator("[data-win]").count()) > 1) { await p.keyboard.press("Alt+Shift+KeyW"); await p.waitForTimeout(150); }
  await p.waitForTimeout(400);
  await p.screenshot({ path: `${dir}/${scheme}-6-single.png` });
  // a neighbour peeking at rest (the focused window fully in view, the next cut cleanly by the canvas edge), the share menu, and two windows for the active and inactive states
  await p.keyboard.press("Alt+Shift+KeyN"); await p.keyboard.press("Alt+Shift+KeyN"); await p.keyboard.press("Alt+Shift+KeyN"); await p.waitForTimeout(500);
  for (let i = 0; i < 6; i++) await p.keyboard.press("Alt+Shift+ArrowLeft");
  for (let i = 0; i < 4; i++) { await p.keyboard.press("Alt+Shift+KeyR"); await p.waitForTimeout(100); await p.keyboard.press("Alt+Shift+ArrowRight"); await p.waitForTimeout(100); }
  for (let i = 0; i < 6; i++) await p.keyboard.press("Alt+Shift+ArrowLeft");
  await p.waitForTimeout(600);
  await p.screenshot({ path: `${dir}/${scheme}-7-peek.png` });
  await p.locator("[data-win][data-focused]").getByRole("button", { name: "Share" }).click(); await p.waitForTimeout(400);
  await p.screenshot({ path: `${dir}/${scheme}-8-share.png` });
  await p.keyboard.press("Escape");
  await ctx.close();
}
{
  const ctx = await ctxOf("dark", true);
  const p = await goto(ctx);
  await p.screenshot({ path: `${dir}/phone-1.png` });
  await ctx.close();
}
await browser.close(); await stop();
console.log("done", dir);
