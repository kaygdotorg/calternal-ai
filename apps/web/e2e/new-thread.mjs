// Palette "New thread" creates a thread through the client (createThread), lists it in the sidebar and opens it.
import { chromium } from "playwright";
import { serve, allThreads, open } from "./lib.mjs";
const stop = await serve(); const b = await chromium.launch();
let bad = 0;
const ok = (n, p, note = "") => { console.log(`${p ? "PASS" : "FAIL"}  ${n}${note ? "  " + note : ""}`); if (!p) bad++; };
const { page, ctx } = await open(b, "desktop", "light", "/threads/t-idle"); await allThreads(page);
const before = await page.locator("nav[aria-label=Threads] a.row").count();
await page.keyboard.press("Control+k");
await page.getByRole("dialog", { name: "Search and commands" }).waitFor();
await page.locator(".pal-input").fill("new thread");
await page.waitForTimeout(200);
await page.locator("[role=option]", { hasText: "New thread" }).first().click();
await page.waitForTimeout(600);
ok("the new thread is open", /\/threads\/t-new-/.test(page.url()), page.url());
ok("the sidebar lists it", (await page.locator("nav[aria-label=Threads] a.row").count()) === before + 1);
ok("its title is on the page", (await page.locator("h1").innerText()).includes("New thread"));
await ctx.close(); await b.close(); stop(); process.exit(bad ? 1 : 0);
