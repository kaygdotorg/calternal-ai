// Snoozing an Inbox thread (docs/threads-ui.md, "Snoozed"): a row's Snooze menu offers later today, tomorrow, next week and a date; the thread leaves the Inbox for a collapsed Snoozed section with its count and when it returns, comes back by itself at that time
// (the page's clock is moved on), and Unsnooze now brings it back at once. Usage: CAL_URL=... node e2e/snooze.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
  await p.clock.install({ time: new Date(2026, 10, 18, 10, 0, 0) }); await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(800);
  const showSnoozed = async () => { const g = p.locator("aside.side .snz .sg"); if ((await g.getAttribute("aria-expanded")) !== "true") await g.click(); await p.waitForTimeout(200); };
  const inbox = () => p.locator("aside.side [role=tabpanel] ul[aria-label='Waiting for you'] > li[data-thread]:has(button[aria-label='Snooze'])");
  const n0 = await inbox().count();
  ok(n0 >= 2, "the Inbox lists threads that wait on the person", String(n0));
  const first = inbox().first();
  const title = (await first.locator(".t").innerText()).trim();
  await first.hover(); await first.getByRole("button", { name: "Snooze" }).click();
  const items = (await p.locator("[role^=menuitem]").allInnerTexts()).map((s) => s.trim());
  ok(items.some((x) => /Later today/.test(x)) && items.some((x) => /Tomorrow, 09:00/.test(x)) && items.some((x) => /Next week/.test(x)) && items.some((x) => /pick a date/i.test(x)), "the menu offers later today, tomorrow, next week and a date", items.join(" | "));
  await p.locator("[role^=menuitem]", { hasText: /Later today/ }).first().click(); await p.waitForTimeout(400);
  ok((await inbox().count()) === n0 - 1, "the thread leaves the Inbox", `${n0} -> ${await inbox().count()}`);
  const sg = p.locator("aside.side .snz .sg");
  ok((await sg.count()) === 1 && /Snoozed\s*1/.test(await sg.innerText()) && (await sg.getAttribute("aria-expanded")) === "false", "a collapsed Snoozed section shows with its count");
  await sg.click(); await p.waitForTimeout(200);
  const row = p.locator("aside.side .snz li").first();
  ok((await row.innerText()).includes(title) && (await row.locator(".tail").getAttribute("title")) === "Back at 13:00", "expanded, it lists the thread in one line and says when it returns (the time, with the full phrase as its tooltip)", (await row.innerText()).replace(/\s+/g, " "));
  // back by itself
  await p.clock.fastForward("04:00:00"); await p.waitForTimeout(500);
  ok((await inbox().count()) === n0 && (await p.locator("aside.side .snz").count()) === 0, "at that time the thread is back in the Inbox on its own");
  // unsnooze now
  const again = inbox().first();
  await again.hover(); await again.getByRole("button", { name: "Snooze" }).click(); await p.locator("[role^=menuitem]", { hasText: /Tomorrow/ }).first().click(); await p.waitForTimeout(300);
  await showSnoozed();
  const snoozedRow = p.locator("aside.side .snz li").first();
  ok((await snoozedRow.locator(".tail").getAttribute("title")) === "Back tomorrow, 09:00", "tomorrow reads \"Back tomorrow, 09:00\"");
  await snoozedRow.hover(); await snoozedRow.getByRole("button", { name: "Unsnooze now" }).click(); await p.waitForTimeout(300);
  ok((await inbox().count()) === n0 && (await p.locator("aside.side .snz").count()) === 0, "Unsnooze now brings it back at once");
  // pick a date
  const third = inbox().first();
  await third.hover(); await third.getByRole("button", { name: "Snooze" }).click(); await p.locator("[role^=menuitem]", { hasText: /pick a date/i }).first().click(); await p.waitForTimeout(300);
  ok((await p.getByLabel("Date and time").count()) === 1, "Pick a date and time opens a date and time field");
  await p.getByLabel("Date and time").fill("2026-11-25T08:30"); await p.locator("[role=dialog]").getByRole("button", { name: "Snooze", exact: true }).click(); await p.waitForTimeout(400);
  await showSnoozed();
  ok((await p.locator("aside.side .snz li .tail").first().getAttribute("title")) === "Back Wed 25 Nov, 08:30", "the chosen date shows in the Snoozed section");
  // the thread's own menu has one entry, Snooze…, that opens the panel with the presets
  await p.locator("aside.side [role=tab]", { hasText: "All Threads" }).click(); await p.waitForTimeout(200);
  const rowAll = p.locator("aside.side [role=tabpanel] .rw").first();
  await rowAll.hover(); await rowAll.getByRole("button", { name: "More actions" }).click();
  const names = (await p.locator("[role^=menuitem]").allInnerTexts()).map((x) => x.trim());
  ok(names.includes("Snooze…") && !names.some((x) => /^Snooze:/.test(x)), "the thread menu carries one Snooze… entry (a long menu would turn into a sheet on a phone)", names.join(" | "));
  await p.locator("[role^=menuitem]", { hasText: "Snooze…" }).click(); await p.waitForTimeout(300);
  ok((await p.locator("[role=dialog] .pre").count()) >= 2, "it opens the snooze panel with the presets and a date");
  await ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
