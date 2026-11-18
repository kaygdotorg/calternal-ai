// A picture of each onboarding step on fixtures, desktop and phone, light and dark (SHOTS=<dir>): for the issue and for looking at.
import { chromium } from "playwright";
import { mkdirSync } from "node:fs";
import { BASE, serve } from "./lib.mjs";
const dir = process.env.SHOTS ?? "/tmp/onboarding-shots"; mkdirSync(dir, { recursive: true });
const stop = await serve(); const b = await chromium.launch();
for (const [dev, vp, mobile] of [["desktop", { width: 1280, height: 860 }, false], ["phone", { width: 390, height: 844 }, true]]) for (const scheme of ["light", "dark"]) {
  const ctx = await b.newContext({ viewport: vp, deviceScaleFactor: 2, isMobile: mobile, hasTouch: mobile, colorScheme: scheme, permissions: ["clipboard-read", "clipboard-write"] });
  const page = await ctx.newPage(); await page.goto(`${BASE}/?server=0&onboarding=1`); await page.waitForSelector("html[data-ready]"); await page.waitForTimeout(900);
  const snap = async (n) => { await page.waitForTimeout(700); await page.screenshot({ path: `${dir}/${dev}-${scheme}-${n}.png` }); };
  await snap("1-you");
  await page.getByLabel("Your name").fill("Kay"); await page.getByRole("button", { name: "Create passkey" }).click(); await page.waitForFunction(() => document.querySelector("main h1")?.textContent === "Choose a look"); await snap("2-look");
  await page.getByRole("radio").nth(1).click(); await page.waitForFunction(() => document.querySelector("main h1")?.textContent === "Your machine"); await snap("3-machine");
  await page.getByRole("button", { name: "Show the script" }).click(); await page.waitForTimeout(2200); await snap("3b-command");
  await page.getByText("dev-machine").waitFor({ timeout: 20000 }); await snap("3c-connected");
  await page.getByRole("button", { name: "Continue" }).click(); await page.waitForFunction(() => document.querySelector("main h1")?.textContent === "Sign in to a provider"); await snap("4-provider");
  await page.getByRole("button", { name: "Sign in", exact: true }).click(); await page.getByRole("link", { name: /sign-in page/ }).waitFor({ timeout: 10000 }); await snap("4a-link-and-code"); await page.getByLabel("Paste the code it gives you").fill("XYZ"); await page.getByRole("button", { name: "Send the code" }).click(); await page.getByText("Signed in").first().waitFor({ timeout: 15000 }); await snap("4b-signed-in");
  await page.getByRole("button", { name: "Continue" }).click(); await page.waitForFunction(() => document.querySelector("main h1")?.textContent === "You are set up"); await snap("5-done");
  await ctx.close();
}
await b.close(); stop();
