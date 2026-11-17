import { chromium } from "playwright";
import { serve, BASE, DEVICES } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
const page = await (await b.newContext({ ...DEVICES.desktop })).newPage();
await page.goto(BASE + "/threads?firstrun=0"); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(800);
await page.getByRole("radio", { name: "Infra" }).click(); await page.waitForTimeout(500);
await page.keyboard.press("Control+k"); await page.waitForSelector(".pal-input"); await page.locator(".pal-input").fill("journal"); await page.waitForTimeout(500);
console.log(JSON.stringify(await page.locator('[role=option]').allInnerTexts()));
await b.close(); stop();
