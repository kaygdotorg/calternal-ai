// Behaviour checks for the owner's list: one-line rows, one state signal, tooltip, title rename, archive + undo toast, pill press.
import { chromium } from "playwright";
import { serve, open } from "./lib.mjs";
const stop = await serve();
const browser = await chromium.launch();
const out = [];
const ok = (name, pass, note = "") => { out.push({ name, pass }); console.log(`${pass ? "PASS" : "FAIL"}  ${name}${note ? "  " + note : ""}`); };
const { page, ctx } = await open(browser, "desktop", "light", "/threads");
const rows = page.locator("a.row");
const n = await rows.count();
const hs = await rows.evaluateAll((els) => els.map((e) => e.getBoundingClientRect().height));
ok("rows are one line (all the same height as a row token)", n > 0 && hs.every((h) => Math.abs(h - hs[0]) < 1) && hs[0] <= 34, `n=${n} h=${hs[0]}`);
const both = await page.evaluate(() => [...document.querySelectorAll(".mark")].filter((m) => m.querySelector("svg, .spinner") && m.textContent.trim()).length);
ok("no state mark carries both icon and word", both === 0);
ok("no automatic workspace labels", (await page.locator("text=/^infraternal$|^calternal-ai$/i").count()) === 0);
// tooltip
const btn = page.getByRole("button", { name: /Search/ }).first();
await btn.hover();
await page.waitForTimeout(250);
const early = await page.locator(".warm-tip[data-open=true]").count();
await page.waitForTimeout(400);
const late = await page.locator(".warm-tip[data-open=true]").count();
ok("tooltip waits ~400 ms then shows", early === 0 && late === 1);
// press: plain CSS (scale to --press-scale and back over --dur-fast); a pointer click starts no rebound animation
await page.mouse.move(5, 5);
await rows.first().click();
await page.waitForSelector("h1");
const pill = page.locator(".composer .chip.prov button").first();
await pill.click();
await page.waitForTimeout(60);
const running = await pill.evaluate((el) => el.getAnimations().filter((a) => !(a instanceof CSSTransition)).length);
ok("a pointer click starts no rebound on the pill (the press is CSS only)", running === 0, `animations=${running}`);
await page.keyboard.press("Escape");
// open a thread, rename by double-clicking the title
await rows.first().click();
await page.waitForSelector("h1");
await page.locator("h1").dblclick();
const field = page.getByLabel("Rename thread");
ok("double-clicking the title opens an inline editor", (await field.count()) === 1);
await field.fill("Renamed here");
await field.press("Enter");
await page.waitForTimeout(100);
ok("rename applies", (await page.locator("h1").innerText()) === "Renamed here");
// archive + undo
await rows.first().click({ button: "right" });
await page.getByRole("menuitem", { name: "Archive" }).click();
await page.waitForTimeout(400);
const toast = page.locator('[aria-label="Notifications"]');
ok("archive shows a toast with Undo", (await toast.getByText("Undo").count()) > 0);
await page.waitForTimeout(700);
await page.screenshot({ path: "/tmp/interact.png", clip: { x: 400, y: 780, width: 800, height: 120 } });
await toast.getByText("Undo").first().click();
await page.waitForTimeout(300);
ok("undo brings the thread back", (await rows.count()) === n);
await ctx.close(); await browser.close(); stop();
process.exit(out.every((o) => o.pass) ? 0 : 1);
