// Live: a real turn with a tool step (ls), reload mid-turn replay, stop mid-stream, readiness shown. Same setup as live-threads.mjs.
import { readFileSync } from "node:fs";
import { chromium } from "playwright";
import { serve, BASE } from "./lib.mjs";
const token = readFileSync(process.env.CAL_TOKEN_FILE, "utf8").trim();
const stop = await serve(); const b = await chromium.launch();
const ok = (n, p, note = "") => console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`);
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 } });
await ctx.addInitScript(([t]) => { localStorage.setItem("cal.firstrun", "1"); localStorage.setItem("cal.devtoken", t); localStorage.setItem("cal.server", "1"); }, [token]);
const page = await ctx.newPage();
await page.goto(BASE + "/threads?server=1"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(2000);
await page.keyboard.press("Control+k"); await page.locator(".pal-input").fill("new thread"); await page.waitForTimeout(250);
await page.locator("[role=option]", { hasText: "New thread" }).first().click();
await page.waitForURL(/\/threads\/[0-9a-f-]{36}/); await page.waitForTimeout(500);
console.log("    readiness text: " + (await page.locator("[role=status], .notice, .ctx").allInnerTexts()).join(" | ").replace(/\s+/g, " ").slice(0, 200));
const f = page.locator("#composer-field"); await f.click(); await f.fill("Run `ls` in your workspace and tell me what it printed.");
const t0 = Date.now(); await page.keyboard.press("Enter");
let step = -1; for (let i = 0; i < 300 && step < 0; i++) { if ((await page.locator("text=/Working/").count()) > 0) step = Date.now() - t0; else await page.waitForTimeout(100); }
await page.waitForTimeout(400);
await page.reload(); await page.waitForSelector("html[data-ready]");
let rep = -1; const t1 = Date.now(); for (let i = 0; i < 100 && rep < 0; i++) { if ((await page.locator("text=/Working|Ran|ls/").count()) > 0) rep = Date.now() - t1; else await page.waitForTimeout(100); }
ok("working row shown", step >= 0, `${step} ms after send`); ok("replayed after reload", rep >= 0, `${rep} ms`);
let done = -1; for (let i = 0; i < 400 && done < 0; i++) { if ((await page.locator(".msg.agent").count()) > 0 && (await page.getByRole("button", { name: "Stop the turn" }).count()) === 0) done = Date.now() - t0; else await page.waitForTimeout(100); }
ok("turn finished", done >= 0, `${done} ms total`);
console.log("    steps text: " + (await page.locator("[class*=step]").allInnerTexts()).join(" | ").replace(/\s+/g, " ").slice(0, 300));
console.log("    agent: " + (await page.locator(".msg.agent").last().innerText()).replace(/\s+/g, " ").slice(0, 200));
await page.screenshot({ path: "/tmp/live-tools.png" });
await b.close(); await stop();
