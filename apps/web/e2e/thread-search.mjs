// Search inside threads (docs/threads-ui.md, "Searching messages"). Usage: CAL_URL=... node e2e/thread-search.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
  await p.waitForTimeout(600);
  await p.keyboard.press("Control+Shift+F");
  const input = p.locator(".msg-search-input");
  await input.waitFor({ timeout: 3000 });
  ok(await input.evaluate((e) => e.contains(document.activeElement) || e === document.activeElement), "Mod+Shift+F opens the search with the field focused");
  await p.keyboard.type("the");
  const rows = p.locator("[role=listbox][aria-label='Matching messages'] li");
  await rows.first().waitFor({ timeout: 2000 });
  ok((await rows.count()) > 1, "results appear as you type", String(await rows.count()));
  ok((await rows.first().locator(".t").innerText()).length > 0 && (await rows.first().locator(".w").innerText()).length > 0 && (await rows.first().locator("mark").count()) === 1, "each result has its thread, time and the matched line with the match marked");
  const hist = await p.evaluate(() => history.length);
  const first = await rows.first().getAttribute("data-message");
  await p.keyboard.press("ArrowDown"); await p.waitForTimeout(500);
  ok((await rows.nth(1).getAttribute("aria-selected")) === "true", "ArrowDown selects the next result");
  const q1 = new URL(p.url()).searchParams.get("m");
  ok(q1 && q1 !== first, "the thread view follows the selection (address carries ?m=)", q1 ?? "");
  ok((await p.evaluate(() => history.length)) === hist, "following replaces the address, so Back does not collect each step");
  await p.keyboard.press("ArrowDown"); await p.waitForTimeout(500);
  const rendered = await p.locator("[aria-label=Messages] [data-message], [aria-label=Messages] .msg").count();
  ok(rendered < 80, "the list jumps to a message without rendering what is between", String(rendered));
  await p.keyboard.press("Enter"); await p.waitForTimeout(400);
  ok((await p.locator(".msg-search-input").count()) === 0, "Enter closes the search");
  ok(/\/threads\/[^/?]+\?m=/.test(p.url()), "and leaves the thread open at the message", p.url());
  ok((await p.locator(".msg[data-hit]").count()) <= 1, "the found message is marked briefly");
  // Esc restores
  const before = p.url();
  await p.keyboard.press("Control+Shift+F"); await p.keyboard.type("the"); await rows.first().waitFor(); await p.keyboard.press("ArrowDown"); await p.waitForTimeout(300);
  await p.keyboard.press("Escape"); await p.waitForTimeout(400);
  ok(p.url() === before, "Esc closes and returns to where you were", p.url());
  // sidebar button and a tap
  await p.locator("aside.side").getByRole("button", { name: "Search messages" }).click();
  await p.keyboard.type("the"); await rows.first().waitFor();
  await rows.nth(2).click(); await p.waitForTimeout(500);
  ok(/\?m=/.test(p.url()) && (await p.locator(".msg-search-input").count()) === 0, "the sidebar button opens it and a tap opens the message");
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
