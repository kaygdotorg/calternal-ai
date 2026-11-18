// Frames of Settings on a desktop card, Wide and Centred, to /tmp/settings-shots (attach to the issue, never commit). Usage: node e2e/with-preview.mjs e2e/settings-shots.mjs [dir]
import { mkdirSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const dir = process.argv[2] ?? "/tmp/settings-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1500, height: 970 }, deviceScaleFactor: 1, colorScheme: "dark" });
await ctx.addInitScript(() => { try { localStorage.setItem("cal.firstrun", "1"); } catch { /* none */ } });
const p = await ctx.newPage();
await p.goto(BASE + "/settings/appearance"); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(2500);
await p.screenshot({ path: `${dir}/wide.png` });
await p.locator("[data-setting='appearance.chat-width']").getByRole("tab", { name: "Centred" }).click(); await p.waitForTimeout(800);
await p.locator(".scroller").evaluate((e) => { e.scrollTop = 0; }); await p.waitForTimeout(400);
await p.screenshot({ path: `${dir}/centred.png` });
await browser.close(); await stop();
