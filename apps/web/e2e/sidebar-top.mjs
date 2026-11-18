// The Threads sidebar's top section and its Inbox / All Threads tabs (docs/threads-ui.md, "The sidebar's top and tabs"). Usage: CAL_URL=... node e2e/sidebar-top.mjs
import { launch, open, serve } from "./lib.mjs";
const stop = await serve(); const browser = await launch();
let bad = 0;
const ok = (c, m, note = "") => { console.log(`${c ? "PASS" : "FAIL"}  ${m}${note ? "  " + note : ""}`); if (!c) bad++; };
const names = (p) => p.locator("aside.side ul[aria-label=Shortcuts] > li[data-top] .row .t, aside.side ul[aria-label=Shortcuts] > li[data-top] .rename").allInnerTexts();
const menu = async (p, entry, item) => { await p.locator(`aside.side li[data-top]:has-text("${entry}")`).click({ button: "right" }); await p.getByRole("menuitem", { name: item }).click(); };
try {
  const { page: p, ctx } = await open(browser, "desktop", "light", "/threads/t-idle");
  await p.waitForTimeout(500);
  ok((await names(p)).join("|") === "New thread|Automations|Skills", "the top section has New thread, Automations and Skills by default", (await names(p)).join("|"));
  ok((await p.locator("aside.side").getByText("Needs you").count()) === 0, "there is no Needs-you card");
  // tabs
  const tabs = p.locator("aside.side [role=tab]");
  ok((await tabs.count()) === 2 && /Inbox/.test(await tabs.nth(0).innerText()) && (await tabs.nth(1).innerText()) === "All Threads", "two tabs: Inbox and All Threads");
  ok((await tabs.nth(0).getAttribute("aria-selected")) === "true", "the Inbox is the first page when threads wait on the person");
  const inbox = await p.locator("aside.side [role=tabpanel] a.row").evaluateAll((a) => a.map((x) => x.textContent.trim()));
  ok(inbox.length > 0 && inbox.every((x) => /question|approval|secret|sign in|failed|interrupted/i.test(x)), "the Inbox lists only threads waiting on the person, each with its reason", inbox.join(" / ").slice(0, 120));
  ok((await p.locator("aside.side [role=tabpanel] .sec").count()) === 0, "the Inbox has no sections");
  await tabs.nth(1).click();
  ok((await p.locator("aside.side [role=tabpanel] .sec").count()) > 0 && (await tabs.nth(1).getAttribute("aria-selected")) === "true", "All Threads shows the list with its sections");
  await tabs.nth(1).focus(); await p.keyboard.press("ArrowLeft");
  ok((await tabs.nth(0).getAttribute("aria-selected")) === "true", "the arrow keys switch tabs");
  // placeholders
  await p.locator("aside.side li[data-top]:has-text('Automations') a").click(); await p.waitForTimeout(300);
  ok(/\/threads\/automations$/.test(p.url()) && (await p.locator("[data-placeholder=automations]").innerText()).includes("not built yet"), "Automations opens a placeholder that says what it is");
  await p.locator("aside.side li[data-top]:has-text('Skills') a").click(); await p.waitForTimeout(300);
  ok((await p.locator("[data-placeholder=skills]").innerText()).includes("not built yet"), "Skills opens a placeholder that says what it is");
  // new thread
  const before = p.url();
  await p.locator("aside.side li[data-top]:has-text('New thread') button.row").click(); await p.waitForTimeout(800);
  ok(/\/threads\/[^/]+$/.test(p.url()) && p.url() !== before && !/automations|skills/.test(p.url()), "New thread starts a thread and opens it", p.url());
  // edit
  await menu(p, "Skills", "Move up"); ok((await names(p)).join("|") === "New thread|Skills|Automations", "Move up reorders");
  await p.locator("aside.side li[data-top]:has-text('Skills') a").focus(); await p.keyboard.press("Alt+ArrowDown");
  ok((await names(p)).join("|") === "New thread|Automations|Skills", "Alt+Down reorders from the keyboard");
  await menu(p, "Automations", "Rename"); await p.keyboard.press("Control+a"); await p.keyboard.type("Routines"); await p.keyboard.press("Enter");
  await menu(p, "Skills", "Remove");
  await p.locator("aside.side").getByRole("button", { name: "Add to this list" }).click(); await p.getByRole("menuitem", { name: "Notes" }).click();
  ok((await names(p)).join("|") === "New thread|Routines|Notes", "rename, remove and add work", (await names(p)).join("|"));
  await p.reload(); await p.waitForSelector("html[data-ready]"); await p.waitForTimeout(500);
  ok((await names(p)).join("|") === "New thread|Routines|Notes", "the arrangement survives a reload");
  await p.locator("aside.side").getByRole("button", { name: "Add to this list" }).click(); await p.getByRole("menuitem", { name: /Restore/ }).click();
  ok((await names(p)).join("|") === "New thread|Automations|Skills", "Restore brings the defaults back");
  await ctx.close();
  // empty inbox
  const e = await open(browser, "desktop", "light", "/threads/t-idle?scenario=quiet");
  await e.page.waitForTimeout(500);
  const t0 = e.page.locator("aside.side [role=tab]");
  const sel = await t0.first().getAttribute("aria-selected");
  const empties = await e.page.locator("aside.side [role=tabpanel]").innerText();
  ok(sel === "true" ? /Nothing is waiting for you/.test(empties) : (await t0.nth(1).getAttribute("aria-selected")) === "true", "with nothing waiting the list opens on All Threads (and the Inbox says so plainly)");
  await t0.first().click();
  ok(/Nothing is waiting for you/.test(await e.page.locator("aside.side [role=tabpanel]").innerText()), "an empty Inbox says nothing is waiting");
  await e.ctx.close();
} catch (err) { console.log("FAIL  " + err.message); bad++; }
await browser.close(); await stop();
process.exit(bad ? 1 : 0);
